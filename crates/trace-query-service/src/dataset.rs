use crate::config::QueryServiceConfig;
use anyhow::Context;
use futures_util::StreamExt;
use reqwest::StatusCode;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use trace_core::lite::s3::parse_s3_uri;
use trace_core::ObjectStore as ObjectStoreTrait;
use uuid::Uuid;

/// Dataset attach/download limits.
///
/// The `_manifest.json` document and referenced objects are treated as **untrusted input**.
/// These limits are defensive (prevent unbounded disk/memory/CPU consumption).
#[derive(Debug, Clone)]
pub struct DatasetDownloadLimits {
    pub max_manifest_bytes: usize,
    pub max_objects: usize,
    pub max_object_bytes: u64,
    pub max_total_bytes: u64,
    pub max_uri_len: usize,
}

impl From<&QueryServiceConfig> for DatasetDownloadLimits {
    fn from(cfg: &QueryServiceConfig) -> Self {
        Self {
            max_manifest_bytes: cfg.dataset_max_manifest_bytes,
            max_objects: cfg.dataset_max_objects,
            max_object_bytes: cfg.dataset_max_object_bytes,
            max_total_bytes: cfg.dataset_max_total_bytes,
            // Conservative URI size; enough for long prefixes, but rejects pathological inputs.
            max_uri_len: 2048,
        }
    }
}

#[derive(Debug)]
pub enum DatasetLoadError {
    /// Permanent, deterministic failure (retries won't help).
    Invalid(anyhow::Error),
    /// Permanent failure due to size/limit enforcement.
    TooLarge(anyhow::Error),
    /// Retryable failure (object store/network/transient IO).
    Retryable(anyhow::Error),
}

impl std::fmt::Display for DatasetLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatasetLoadError::Invalid(err) => write!(f, "dataset invalid: {err}"),
            DatasetLoadError::TooLarge(err) => write!(f, "dataset too large: {err}"),
            DatasetLoadError::Retryable(err) => write!(f, "dataset unavailable: {err}"),
        }
    }
}

impl std::error::Error for DatasetLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DatasetLoadError::Invalid(err)
            | DatasetLoadError::TooLarge(err)
            | DatasetLoadError::Retryable(err) => Some(err.as_ref()),
        }
    }
}

impl DatasetLoadError {
    pub fn category(&self) -> &'static str {
        match self {
            DatasetLoadError::Invalid(_) => "permanent",
            DatasetLoadError::TooLarge(_) => "permanent",
            DatasetLoadError::Retryable(_) => "retryable",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DatasetManifest {
    #[serde(default, alias = "objects")]
    pub parquet_objects: Vec<String>,
}

pub fn manifest_uri(storage_prefix: &str) -> String {
    let prefix = storage_prefix.trim_end_matches('/');
    format!("{prefix}/_manifest.json")
}

pub async fn fetch_manifest(
    object_store: &dyn ObjectStoreTrait,
    storage_prefix: &str,
    limits: &DatasetDownloadLimits,
) -> Result<DatasetManifest, DatasetLoadError> {
    let uri = manifest_uri(storage_prefix);
    let (bucket, key) = parse_s3_uri(&uri)
        .context("parse manifest s3 uri")
        .map_err(DatasetLoadError::Invalid)?;
    let bytes = object_store
        .get_bytes(&bucket, &key)
        .await
        .context("fetch dataset manifest")
        .map_err(DatasetLoadError::Retryable)?;

    if bytes.len() > limits.max_manifest_bytes {
        return Err(DatasetLoadError::TooLarge(anyhow::anyhow!(
            "manifest exceeds max_manifest_bytes: {} > {}",
            bytes.len(),
            limits.max_manifest_bytes
        )));
    }

    let manifest: DatasetManifest = serde_json::from_slice(&bytes)
        .context("parse manifest json")
        .map_err(DatasetLoadError::Invalid)?;

    validate_manifest(&manifest, limits)?;
    Ok(manifest)
}

pub struct DownloadedParquetDataset {
    _dir: TempDir,
    pub parquet_glob: PathBuf,
}

pub async fn download_parquet_objects(
    s3_endpoint: &str,
    http_client: &reqwest::Client,
    parquet_objects: &[String],
    limits: &DatasetDownloadLimits,
) -> Result<DownloadedParquetDataset, DatasetLoadError> {
    if parquet_objects.is_empty() {
        return Err(DatasetLoadError::Invalid(anyhow::anyhow!(
            "manifest has no parquet objects"
        )));
    }

    if parquet_objects.len() > limits.max_objects {
        return Err(DatasetLoadError::TooLarge(anyhow::anyhow!(
            "manifest object count exceeds max_objects: {} > {}",
            parquet_objects.len(),
            limits.max_objects
        )));
    }

    let dir = TempDir::new("trace-query-service-parquet")
        .map_err(DatasetLoadError::Retryable)?;

    let mut remaining_budget = limits.max_total_bytes;

    for (idx, uri) in parquet_objects.iter().enumerate() {
        if uri.len() > limits.max_uri_len {
            return Err(DatasetLoadError::Invalid(anyhow::anyhow!(
                "parquet uri exceeds max_uri_len"
            )));
        }

        let (bucket, key) = parse_s3_uri(uri)
            .context("parse parquet s3 uri")
            .map_err(DatasetLoadError::Invalid)?;

        let file_name = format!("part-{idx:03}.parquet");
        let path = dir.path().join(file_name);

        download_object_to_file(
            s3_endpoint,
            http_client,
            &bucket,
            &key,
            &path,
            limits.max_object_bytes,
            &mut remaining_budget,
        )
        .await?;
    }

    Ok(DownloadedParquetDataset {
        parquet_glob: dir.path().join("*.parquet"),
        _dir: dir,
    })
}

fn validate_manifest(
    manifest: &DatasetManifest,
    limits: &DatasetDownloadLimits,
) -> Result<(), DatasetLoadError> {
    if manifest.parquet_objects.is_empty() {
        return Err(DatasetLoadError::Invalid(anyhow::anyhow!(
            "manifest has no parquet objects"
        )));
    }

    if manifest.parquet_objects.len() > limits.max_objects {
        return Err(DatasetLoadError::TooLarge(anyhow::anyhow!(
            "manifest object count exceeds max_objects: {} > {}",
            manifest.parquet_objects.len(),
            limits.max_objects
        )));
    }

    for uri in &manifest.parquet_objects {
        if uri.len() > limits.max_uri_len {
            return Err(DatasetLoadError::Invalid(anyhow::anyhow!(
                "parquet uri exceeds max_uri_len"
            )));
        }

        // This is a structural validation only (authz is enforced in the API layer).
        parse_s3_uri(uri)
            .context("parquet object must be s3://")
            .map_err(DatasetLoadError::Invalid)?;
    }

    Ok(())
}

async fn download_object_to_file(
    s3_endpoint: &str,
    http_client: &reqwest::Client,
    bucket: &str,
    key: &str,
    dest_path: &Path,
    max_object_bytes: u64,
    remaining_budget: &mut u64,
) -> Result<u64, DatasetLoadError> {
    let url = object_url(s3_endpoint, bucket, key).map_err(DatasetLoadError::Invalid)?;

    let resp = http_client
        .get(url.clone())
        .send()
        .await
        .with_context(|| format!("GET {url}"))
        .map_err(DatasetLoadError::Retryable)?;

    let status = resp.status();
    if !status.is_success() {
        let err = anyhow::anyhow!("GET {url} returned {status}");
        if status == StatusCode::NOT_FOUND || status.is_server_error() {
            return Err(DatasetLoadError::Retryable(err));
        }
        return Err(DatasetLoadError::Invalid(err));
    }

    if let Some(content_len) = resp.content_length() {
        if content_len > max_object_bytes {
            return Err(DatasetLoadError::TooLarge(anyhow::anyhow!(
                "object exceeds max_object_bytes: {content_len} > {max_object_bytes}"
            )));
        }
        if content_len > *remaining_budget {
            return Err(DatasetLoadError::TooLarge(anyhow::anyhow!(
                "dataset exceeds max_total_bytes: {content_len} > {remaining_budget}"
            )));
        }
    }

    let tmp_path = dest_path.with_extension("parquet.part");
    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .with_context(|| format!("create temp file {}", tmp_path.display()))
        .map_err(DatasetLoadError::Retryable)?;

    let mut written: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(next) = stream.next().await {
        let chunk = next
            .context("read object chunk")
            .map_err(DatasetLoadError::Retryable)?;

        written = written.saturating_add(chunk.len() as u64);
        if written > max_object_bytes {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(DatasetLoadError::TooLarge(anyhow::anyhow!(
                "object exceeds max_object_bytes"
            )));
        }
        if written > *remaining_budget {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(DatasetLoadError::TooLarge(anyhow::anyhow!(
                "dataset exceeds max_total_bytes"
            )));
        }

        file.write_all(&chunk)
            .await
            .context("write object chunk")
            .map_err(DatasetLoadError::Retryable)?;
    }
    file.flush()
        .await
        .context("flush parquet file")
        .map_err(DatasetLoadError::Retryable)?;

    tokio::fs::rename(&tmp_path, dest_path)
        .await
        .with_context(|| format!("rename {} -> {}", tmp_path.display(), dest_path.display()))
        .map_err(DatasetLoadError::Retryable)?;

    *remaining_budget = remaining_budget.saturating_sub(written);
    Ok(written)
}

fn object_url(endpoint: &str, bucket: &str, key: &str) -> anyhow::Result<reqwest::Url> {
    let base = endpoint.trim_end_matches('/');
    let full = format!("{base}/{bucket}/{key}");
    Ok(full.parse().context("build object url")?)
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> anyhow::Result<Self> {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("create temp dir {}", dir.display()))?;
        Ok(Self { path: dir })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
