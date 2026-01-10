use crate::dispatcher_client::{CompleteRequest, DispatcherClient};
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    io::{Read, Write},
    path::Path,
    process::Stdio,
    time::Duration,
};
use tempfile::TempDir;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use trace_core::{
    runtime::RuntimeInvoker,
    udf::UdfInvocationPayload,
    udf_bundle::{UdfBundleLimits, UdfBundleManifestV1, UdfBundleRuntime, BUNDLE_MANIFEST_FILE},
    Error, Result as CoreResult,
};
use zip::ZipArchive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalInvokeErrorKind {
    Retryable,
    Permanent,
}

#[derive(Debug)]
struct LocalInvokeError {
    kind: LocalInvokeErrorKind,
    error: anyhow::Error,
}

impl LocalInvokeError {
    fn retryable(err: anyhow::Error) -> Self {
        Self {
            kind: LocalInvokeErrorKind::Retryable,
            error: err,
        }
    }

    fn permanent(err: anyhow::Error) -> Self {
        Self {
            kind: LocalInvokeErrorKind::Permanent,
            error: err,
        }
    }
}

#[derive(Clone)]
pub struct LocalProcessInvoker {
    dispatcher: DispatcherClient,
    http: reqwest::Client,
    node_bin: String,
    python_bin: String,

    dispatcher_url: String,
    query_service_url: String,
    object_store_endpoint: String,
    object_store_bucket: String,

    bundle_max_bytes: u64,
    manifest_max_bytes: u64,
    limits: UdfBundleLimits,

    timeout: Duration,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
}

impl LocalProcessInvoker {
    pub fn new(
        dispatcher_url: String,
        query_service_url: String,
        object_store_endpoint: String,
        object_store_bucket: String,
    ) -> Self {
        let node_bin = std::env::var("TRACE_NODE_BIN").unwrap_or_else(|_| "node".to_string());
        let python_bin =
            std::env::var("TRACE_PYTHON_BIN").unwrap_or_else(|_| "python3".to_string());

        Self {
            dispatcher: DispatcherClient::new(dispatcher_url.clone()),
            http: reqwest::Client::new(),
            node_bin,
            python_bin,
            dispatcher_url,
            query_service_url,
            object_store_endpoint,
            object_store_bucket,
            bundle_max_bytes: 64 * 1024 * 1024,
            manifest_max_bytes: 64 * 1024,
            limits: UdfBundleLimits::default(),
            timeout: Duration::from_secs(30),
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
        }
    }

    async fn invoke_inner(
        &self,
        invocation: &UdfInvocationPayload,
    ) -> std::result::Result<(), LocalInvokeError> {
        let workspace = TempDir::new()
            .context("create udf workspace dir")
            .map_err(|e| LocalInvokeError::retryable(e.into()))?;
        let zip_path = workspace.path().join("bundle.zip");

        self.download_bundle(&invocation.bundle_url, &zip_path)
            .await?;

        let extracted_dir = workspace.path().join("bundle");
        tokio::fs::create_dir_all(&extracted_dir)
            .await
            .context("create extracted dir")
            .map_err(|e| LocalInvokeError::retryable(e.into()))?;

        let manifest = tokio::task::spawn_blocking({
            let zip_path = zip_path.clone();
            let extracted_dir = extracted_dir.clone();
            let limits = self.limits.clone();
            let manifest_max_bytes = self.manifest_max_bytes;
            move || extract_and_validate_zip(&zip_path, &extracted_dir, manifest_max_bytes, &limits)
        })
        .await
        .context("join bundle extract task")
        .map_err(|e| LocalInvokeError::retryable(e.into()))?
        .context("extract and validate bundle")
        .map_err(|e| LocalInvokeError::permanent(e.into()))?;

        self.exec_entrypoint(invocation, &extracted_dir, &manifest)
            .await
            .context("execute bundle entrypoint")
            .map_err(|e| LocalInvokeError::permanent(e.into()))?;

        Ok(())
    }

    async fn download_bundle(
        &self,
        url: &str,
        dest: &Path,
    ) -> std::result::Result<(), LocalInvokeError> {
        if let Some(path) = url.strip_prefix("file://") {
            let src = Path::new(path);
            let meta = tokio::fs::metadata(src)
                .await
                .with_context(|| format!("stat bundle file {}", src.display()))
                .map_err(|e| LocalInvokeError::permanent(e.into()))?;

            if meta.len() > self.bundle_max_bytes {
                return Err(LocalInvokeError::permanent(anyhow!(
                    "bundle exceeds max bytes: {} > {}",
                    meta.len(),
                    self.bundle_max_bytes
                )));
            }

            tokio::fs::copy(src, dest)
                .await
                .with_context(|| {
                    format!("copy bundle file {} -> {}", src.display(), dest.display())
                })
                .map_err(|e| LocalInvokeError::permanent(e.into()))?;
            return Ok(());
        }

        let resp = self
            .http
            .get(url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))
            .map_err(|e| LocalInvokeError::retryable(e.into()))?;

        if let Err(err) = resp.error_for_status_ref() {
            let status = resp.status();
            let kind = if status.is_server_error() {
                LocalInvokeErrorKind::Retryable
            } else {
                LocalInvokeErrorKind::Permanent
            };
            return Err(match kind {
                LocalInvokeErrorKind::Retryable => LocalInvokeError::retryable(err.into()),
                LocalInvokeErrorKind::Permanent => LocalInvokeError::permanent(err.into()),
            });
        }

        let mut resp = resp;

        let mut file = tokio::fs::File::create(dest)
            .await
            .with_context(|| format!("create bundle file {}", dest.display()))
            .map_err(|e| LocalInvokeError::retryable(e.into()))?;

        let mut total: u64 = 0;
        while let Some(chunk) = resp
            .chunk()
            .await
            .context("read response chunk")
            .map_err(|e| LocalInvokeError::retryable(e.into()))?
        {
            total = total
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| LocalInvokeError::permanent(anyhow!("bundle size overflow")))?;
            if total > self.bundle_max_bytes {
                return Err(LocalInvokeError::permanent(anyhow!(
                    "bundle exceeds max bytes: {} > {}",
                    total,
                    self.bundle_max_bytes
                )));
            }
            file.write_all(&chunk)
                .await
                .context("write bundle chunk")
                .map_err(|e| LocalInvokeError::retryable(e.into()))?;
        }
        file.flush()
            .await
            .context("flush bundle")
            .map_err(|e| LocalInvokeError::retryable(e.into()))?;
        Ok(())
    }

    async fn exec_entrypoint(
        &self,
        invocation: &UdfInvocationPayload,
        extracted_dir: &Path,
        manifest: &UdfBundleManifestV1,
    ) -> anyhow::Result<()> {
        let entrypoint_path = extracted_dir.join(&manifest.entrypoint);
        if !entrypoint_path.exists() {
            return Err(anyhow!(
                "entrypoint does not exist: {}",
                entrypoint_path.display()
            ));
        }

        let runtime_bin = match manifest.runtime {
            UdfBundleRuntime::Node => &self.node_bin,
            UdfBundleRuntime::Python => &self.python_bin,
        };

        let invocation_bytes =
            serde_json::to_vec(invocation).context("encode invocation payload as json")?;

        let mut cmd = Command::new(runtime_bin);
        cmd.current_dir(extracted_dir)
            .arg(&manifest.entrypoint)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear();

        if let Ok(path) = std::env::var("PATH") {
            cmd.env("PATH", path);
        }

        cmd.env("TRACE_DISPATCHER_URL", &self.dispatcher_url);
        cmd.env("TRACE_QUERY_SERVICE_URL", &self.query_service_url);
        cmd.env("TRACE_OBJECT_STORE_ENDPOINT", &self.object_store_endpoint);
        cmd.env("TRACE_OBJECT_STORE_BUCKET", &self.object_store_bucket);

        let mut child = cmd.spawn().context("spawn udf process")?;

        let mut stdin = child.stdin.take().context("take stdin")?;
        stdin
            .write_all(&invocation_bytes)
            .await
            .context("write invocation to stdin")?;
        stdin.shutdown().await.context("close stdin")?;

        let stdout = child.stdout.take().context("take stdout")?;
        let stderr = child.stderr.take().context("take stderr")?;

        let stdout_task =
            tokio::spawn(read_stream_limited(stdout, self.max_stdout_bytes, "stdout"));
        let stderr_task =
            tokio::spawn(read_stream_limited(stderr, self.max_stderr_bytes, "stderr"));

        let status = match tokio::time::timeout(self.timeout, child.wait()).await {
            Ok(res) => res.context("wait udf process")?,
            Err(_) => {
                stdout_task.abort();
                stderr_task.abort();
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(anyhow!("udf process timed out"));
            }
        };

        let stdout_bytes = stdout_task
            .await
            .context("join stdout task")?
            .context("read stdout")?;
        let stderr_bytes = stderr_task
            .await
            .context("join stderr task")?
            .context("read stderr")?;

        if !status.success() {
            let stderr = String::from_utf8_lossy(&stderr_bytes);
            return Err(anyhow!("udf process failed: {}", stderr.trim()));
        }

        let value: serde_json::Value =
            serde_json::from_slice(&stdout_bytes).context("parse stdout as json")?;
        if !value.is_object() {
            return Err(anyhow!("udf stdout must be a JSON object"));
        }

        Ok(())
    }

    async fn fail_attempt(
        &self,
        invocation: &UdfInvocationPayload,
        outcome: &'static str,
    ) -> anyhow::Result<()> {
        let req = CompleteRequest {
            task_id: invocation.task_id,
            attempt: invocation.attempt,
            lease_token: invocation.lease_token,
            outcome,
            datasets_published: vec![],
        };
        self.dispatcher
            .complete(&invocation.capability_token, &req)
            .await
            .context("complete task")
            .map(|_| ())
    }
}

#[async_trait]
impl RuntimeInvoker for LocalProcessInvoker {
    async fn invoke(&self, invocation: &UdfInvocationPayload) -> CoreResult<()> {
        match self.invoke_inner(invocation).await {
            Ok(()) => Ok(()),
            Err(err) => {
                tracing::warn!(
                    event = "harness.udf.local.failed",
                    task_id = %invocation.task_id,
                    attempt = invocation.attempt,
                    error = %err.error,
                    "local udf invocation failed"
                );

                let outcome = if err.kind == LocalInvokeErrorKind::Retryable {
                    "retryable_error"
                } else {
                    "fatal_error"
                };

                if let Err(complete_err) = self.fail_attempt(invocation, outcome).await {
                    return Err(Error::from(anyhow::anyhow!(complete_err)));
                }
                Ok(())
            }
        }
    }
}

async fn read_stream_limited<R: AsyncRead + Unpin>(
    mut reader: R,
    max_bytes: usize,
    label: &'static str,
) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = reader.read(&mut chunk).await.context("read")?;
        if n == 0 {
            break;
        }
        if buf.len() + n > max_bytes {
            return Err(anyhow!("{label} exceeds max bytes: {}", max_bytes));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(buf)
}

fn extract_and_validate_zip(
    zip_path: &Path,
    extracted_dir: &Path,
    manifest_max_bytes: u64,
    limits: &UdfBundleLimits,
) -> anyhow::Result<UdfBundleManifestV1> {
    let file = std::fs::File::open(zip_path)
        .with_context(|| format!("open zip {}", zip_path.display()))?;
    let mut archive = ZipArchive::new(file).context("open zip archive")?;

    let manifest_bytes = {
        let mut manifest_entry = archive
            .by_name(BUNDLE_MANIFEST_FILE)
            .context("missing bundle_manifest.json")?;

        if manifest_entry.size() > manifest_max_bytes {
            return Err(anyhow!(
                "bundle manifest too large: {} > {}",
                manifest_entry.size(),
                manifest_max_bytes
            ));
        }

        let mut manifest_bytes = Vec::with_capacity(manifest_entry.size() as usize);
        manifest_entry
            .read_to_end(&mut manifest_bytes)
            .context("read bundle manifest")?;
        manifest_bytes
    };

    let manifest: UdfBundleManifestV1 =
        serde_json::from_slice(&manifest_bytes).context("decode bundle manifest")?;
    manifest
        .validate(limits)
        .map_err(|e| anyhow!("manifest validation failed: {e}"))?;

    let declared: HashSet<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();

    let archive_len = archive.len();
    for i in 0..archive_len {
        let entry = archive.by_index(i).context("read zip entry")?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name();
        if name == BUNDLE_MANIFEST_FILE {
            continue;
        }
        if !declared.contains(name) {
            return Err(anyhow!("zip contains undeclared file '{name}'"));
        }
    }

    for file in &manifest.files {
        let mut entry = archive
            .by_name(&file.path)
            .with_context(|| format!("missing file '{}' in zip", file.path))?;
        let out_path = extracted_dir.join(&file.path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }

        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&out_path)
            .with_context(|| format!("create file {}", out_path.display()))?;

        let mut hasher = Sha256::new();
        let mut written: u64 = 0;
        let mut buf = [0u8; 8192];
        loop {
            let n = entry.read(&mut buf).context("read zip file bytes")?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n]).context("write extracted bytes")?;
            written = written
                .checked_add(n as u64)
                .ok_or_else(|| anyhow!("file too large"))?;
            if written > file.bytes {
                return Err(anyhow!(
                    "file '{}' exceeds declared bytes: {} > {}",
                    file.path,
                    written,
                    file.bytes
                ));
            }
        }
        out.flush().context("flush extracted file")?;

        if written != file.bytes {
            return Err(anyhow!(
                "file '{}' bytes mismatch: {} != {}",
                file.path,
                written,
                file.bytes
            ));
        }

        let digest = hasher.finalize();
        let got = hex_lower(&digest);
        if got != file.sha256 {
            return Err(anyhow!(
                "file '{}' sha256 mismatch: {} != {}",
                file.path,
                got,
                file.sha256
            ));
        }
    }

    Ok(manifest)
}

fn hex_lower(bytes: &[u8]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(LUT[(b >> 4) as usize] as char);
        out.push(LUT[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::io::Cursor;
    use trace_core::udf::UdfInvocationPayload;
    use uuid::Uuid;

    fn make_zip(files: Vec<(&str, &[u8])>) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let cursor = Cursor::new(&mut buf);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            for (path, bytes) in files {
                zip.start_file(path, options).unwrap();
                zip.write_all(bytes).unwrap();
            }

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn extract_rejects_undeclared_files() {
        let manifest = serde_json::json!({
            "schema_version": 1,
            "runtime": "node",
            "entrypoint": "index.js",
            "files": [{
                "path": "index.js",
                "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                "bytes": 0
            }]
        });

        let zip_bytes = make_zip(vec![
            (
                BUNDLE_MANIFEST_FILE,
                serde_json::to_string(&manifest).unwrap().as_bytes(),
            ),
            ("index.js", b""),
            ("extra.js", b"console.log('nope')"),
        ]);

        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("bundle.zip");
        std::fs::write(&zip_path, zip_bytes).unwrap();
        let out_dir = tmp.path().join("out");
        std::fs::create_dir_all(&out_dir).unwrap();

        let err =
            extract_and_validate_zip(&zip_path, &out_dir, 64 * 1024, &UdfBundleLimits::default())
                .unwrap_err();
        assert!(err.to_string().contains("undeclared file"));
    }

    #[test]
    fn extract_rejects_sha_mismatch() {
        let manifest = serde_json::json!({
            "schema_version": 1,
            "runtime": "node",
            "entrypoint": "index.js",
            "files": [{
                "path": "index.js",
                "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                "bytes": 0
            }]
        });

        let zip_bytes = make_zip(vec![
            (
                BUNDLE_MANIFEST_FILE,
                serde_json::to_string(&manifest).unwrap().as_bytes(),
            ),
            ("index.js", b""),
        ]);

        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("bundle.zip");
        std::fs::write(&zip_path, zip_bytes).unwrap();
        let out_dir = tmp.path().join("out");
        std::fs::create_dir_all(&out_dir).unwrap();

        let err =
            extract_and_validate_zip(&zip_path, &out_dir, 64 * 1024, &UdfBundleLimits::default())
                .unwrap_err();
        assert!(err.to_string().contains("sha256 mismatch"));
    }

    #[tokio::test]
    async fn local_process_invoker_smoke_node_bundle() -> anyhow::Result<()> {
        if std::env::var("RUN_REAL_UDF_TESTS").ok().as_deref() != Some("1") {
            return Ok(());
        }

        let node_bin = std::env::var("TRACE_NODE_BIN").unwrap_or_else(|_| "node".to_string());
        tokio::process::Command::new(&node_bin)
            .arg("--version")
            .output()
            .await
            .with_context(|| format!("RUN_REAL_UDF_TESTS=1 but {node_bin} is not runnable"))?;

        let script =
            br#"require('fs').readFileSync(0, 'utf8'); console.log(JSON.stringify({ ok: true }));"#;
        let sha = {
            let mut hasher = Sha256::new();
            hasher.update(script);
            hex_lower(&hasher.finalize())
        };

        let manifest = serde_json::json!({
            "schema_version": 1,
            "runtime": "node",
            "entrypoint": "index.js",
            "files": [{
                "path": "index.js",
                "sha256": sha,
                "bytes": script.len(),
            }]
        });
        let manifest_str = serde_json::to_string(&manifest).context("encode manifest")?;

        let zip_bytes = make_zip(vec![
            (BUNDLE_MANIFEST_FILE, manifest_str.as_bytes()),
            ("index.js", script),
        ]);

        let tmp = tempfile::tempdir().context("create temp dir")?;
        let zip_path = tmp.path().join("bundle.zip");
        tokio::fs::write(&zip_path, zip_bytes)
            .await
            .context("write zip")?;

        let invoker = LocalProcessInvoker::new(
            "http://127.0.0.1:0".to_string(),
            "http://127.0.0.1:0".to_string(),
            "http://127.0.0.1:0".to_string(),
            "trace-harness".to_string(),
        );

        let invocation = UdfInvocationPayload {
            task_id: Uuid::new_v4(),
            attempt: 1,
            lease_token: Uuid::new_v4(),
            lease_expires_at: Utc::now(),
            capability_token: "test".to_string(),
            bundle_url: format!("file://{}", zip_path.display()),
            work_payload: serde_json::json!({}),
        };

        invoker.invoke(&invocation).await?;
        Ok(())
    }
}
