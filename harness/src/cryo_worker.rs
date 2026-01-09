use crate::config::HarnessConfig;
use crate::dispatcher_client::{CompleteRequest, DispatcherClient, WriteDisposition};
use crate::pgqueue::PgQueue;
use crate::s3::ObjectStore;
use anyhow::Context;
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};
use trace_core::{
    manifest::DatasetManifestV1, DatasetPublication, DatasetStorageRef,
    ObjectStore as ObjectStoreTrait, Queue as QueueTrait,
};
use uuid::Uuid;

const CONTENT_TYPE_PARQUET: &str = "application/octet-stream";
const CONTENT_TYPE_JSON: &str = "application/json";

const DEFAULT_STAGING_TTL_HOURS: u64 = 24;

const DEFAULT_MAX_PARQUET_FILES_PER_RANGE: usize = 256;
const DEFAULT_MAX_PARQUET_BYTES_PER_FILE: u64 = 512 * 1024 * 1024;
const DEFAULT_MAX_TOTAL_PARQUET_BYTES_PER_RANGE: u64 = 2 * 1024 * 1024 * 1024;

// UUIDv5 namespace for deterministic dataset version IDs.
const DATASET_VERSION_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6e, 0x48, 0x4f, 0x2c, 0x56, 0x7a, 0x44, 0xf3, 0x8a, 0x5e, 0xc0, 0x65, 0x0b, 0x2a, 0x16, 0x9f,
]);

#[derive(Debug, Clone)]
struct CryoArtifactCaps {
    max_parquet_files_per_range: usize,
    max_parquet_bytes_per_file: u64,
    max_total_parquet_bytes_per_range: u64,
}

impl CryoArtifactCaps {
    fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            max_parquet_files_per_range: parse_env_usize(
                "MAX_PARQUET_FILES_PER_RANGE",
                DEFAULT_MAX_PARQUET_FILES_PER_RANGE,
            )?,
            max_parquet_bytes_per_file: parse_env_u64(
                "MAX_PARQUET_BYTES_PER_FILE",
                DEFAULT_MAX_PARQUET_BYTES_PER_FILE,
            )?,
            max_total_parquet_bytes_per_range: parse_env_u64(
                "MAX_TOTAL_PARQUET_BYTES_PER_RANGE",
                DEFAULT_MAX_TOTAL_PARQUET_BYTES_PER_RANGE,
            )?,
        })
    }
}

fn parse_env_u64(key: &str, default: u64) -> anyhow::Result<u64> {
    match std::env::var(key) {
        Ok(v) => v.parse::<u64>().with_context(|| format!("parse {key}={v}")),
        Err(_) => Ok(default),
    }
}

fn parse_env_usize(key: &str, default: usize) -> anyhow::Result<usize> {
    match std::env::var(key) {
        Ok(v) => v
            .parse::<usize>()
            .with_context(|| format!("parse {key}={v}")),
        Err(_) => Ok(default),
    }
}

#[derive(Debug, Deserialize)]
struct TaskWakeup {
    task_id: Uuid,
}

#[derive(Debug)]
enum CryoArtifactError {
    Retryable(anyhow::Error),
    Fatal(anyhow::Error),
}

impl CryoArtifactError {
    fn outcome(&self) -> &'static str {
        match self {
            CryoArtifactError::Retryable(_) => "retryable_error",
            CryoArtifactError::Fatal(_) => "fatal_error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryoIngestPayload {
    pub dataset_uuid: Uuid,
    pub chain_id: i64,
    pub range_start: i64,
    /// End-exclusive block range end (`[range_start, range_end)`).
    pub range_end: i64,
    pub config_hash: String,
    #[serde(default)]
    pub dataset_key: Option<String>,
    #[serde(default)]
    pub cryo_dataset_name: Option<String>,
    #[serde(default)]
    pub rpc_pool: Option<String>,
}

pub fn derive_dataset_publication(bucket: &str, payload: &CryoIngestPayload) -> DatasetPublication {
    let dataset_version = derive_dataset_version(payload);
    let prefix = format!(
        "cryo/{}/{}/{}_{}/{}/",
        payload.chain_id,
        payload.dataset_uuid,
        payload.range_start,
        payload.range_end,
        dataset_version
    );

    DatasetPublication {
        dataset_uuid: payload.dataset_uuid,
        dataset_version,
        storage_ref: DatasetStorageRef::S3 {
            bucket: bucket.to_string(),
            prefix,
            glob: "*.parquet".to_string(),
        },
        config_hash: payload.config_hash.clone(),
        range_start: payload.range_start,
        range_end: payload.range_end,
    }
}

pub async fn run_task(
    cfg: &HarnessConfig,
    object_store: &dyn ObjectStoreTrait,
    dispatcher: &DispatcherClient,
    task_id: Uuid,
) -> anyhow::Result<Option<DatasetPublication>> {
    let caps = CryoArtifactCaps::from_env().context("parse cryo artifact caps")?;
    run_task_with_caps(cfg, object_store, dispatcher, task_id, &caps).await
}

async fn run_task_with_caps(
    cfg: &HarnessConfig,
    object_store: &dyn ObjectStoreTrait,
    dispatcher: &DispatcherClient,
    task_id: Uuid,
    caps: &CryoArtifactCaps,
) -> anyhow::Result<Option<DatasetPublication>> {
    let Some(claim) = dispatcher.task_claim(task_id).await? else {
        return Ok(None);
    };

    let payload: CryoIngestPayload =
        serde_json::from_value(claim.work_payload.clone()).context("decode cryo payload")?;

    let pubd = derive_dataset_publication(&cfg.s3_bucket, &payload);

    match write_dataset_artifacts(
        object_store,
        &pubd,
        &payload,
        claim.task_id,
        claim.attempt,
        caps,
    )
    .await
    {
        Ok(()) => {
            let complete_req = CompleteRequest {
                task_id: claim.task_id,
                attempt: claim.attempt,
                lease_token: claim.lease_token,
                outcome: "success",
                datasets_published: vec![pubd.clone()],
            };

            if dispatcher
                .complete(&claim.capability_token, &complete_req)
                .await?
                == WriteDisposition::Conflict
            {
                return Ok(None);
            }

            Ok(Some(pubd))
        }
        Err(err) => {
            tracing::warn!(
                event = "harness.cryo_worker.task.failed",
                outcome = err.outcome(),
                task_id = %claim.task_id,
                attempt = claim.attempt,
                error = %match &err { CryoArtifactError::Retryable(e) | CryoArtifactError::Fatal(e) => e },
                "cryo task failed"
            );

            let complete_req = CompleteRequest {
                task_id: claim.task_id,
                attempt: claim.attempt,
                lease_token: claim.lease_token,
                outcome: err.outcome(),
                datasets_published: Vec::new(),
            };

            if dispatcher
                .complete(&claim.capability_token, &complete_req)
                .await?
                == WriteDisposition::Conflict
            {
                return Ok(None);
            }

            Ok(None)
        }
    }
}

pub async fn run(cfg: &HarnessConfig) -> anyhow::Result<()> {
    let staging_root = staging_root();
    ensure_private_dir(&staging_root)
        .await
        .context("ensure cryo staging root")?;

    cleanup_stale_staging_dirs(
        staging_root.clone(),
        Duration::from_secs(
            std::env::var("TRACE_CRYO_STAGING_TTL_HOURS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(DEFAULT_STAGING_TTL_HOURS)
                * 3600,
        ),
    )
    .await
    .context("cleanup stale cryo staging dirs")?;

    let caps = CryoArtifactCaps::from_env().context("parse cryo artifact caps")?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;
    let queue: Arc<dyn QueueTrait> = Arc::new(PgQueue::new(pool));

    let object_store: Arc<dyn ObjectStoreTrait> =
        Arc::new(ObjectStore::new(&cfg.s3_endpoint).context("init object store")?);
    let dispatcher = DispatcherClient::new(cfg.dispatcher_url.clone());

    let poll_interval = Duration::from_millis(cfg.worker_poll_ms);
    let visibility_timeout = Duration::from_secs(cfg.worker_visibility_timeout_secs);
    let requeue_delay = Duration::from_millis(cfg.worker_requeue_delay_ms);

    tracing::info!(
        event = "harness.cryo_worker.started",
        queue = %cfg.task_wakeup_queue,
        dispatcher = %cfg.dispatcher_url,
        "cryo worker started"
    );

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!(event = "harness.cryo_worker.shutdown", "cryo worker shutting down");
                return Ok(());
            }
            res = queue.receive(&cfg.task_wakeup_queue, 1, visibility_timeout) => {
                let messages = res?;
                if messages.is_empty() {
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }

                for msg in messages {
                    if let Err(err) = handle_message(cfg, queue.as_ref(), object_store.as_ref(), &dispatcher, msg, requeue_delay, &caps).await {
                        tracing::warn!(
                            event = "harness.cryo_worker.message.error",
                            error = %err,
                            "cryo worker message handling failed"
                        );
                    }
                }
            }
        }
    }
}

async fn handle_message(
    cfg: &HarnessConfig,
    queue: &dyn QueueTrait,
    object_store: &dyn ObjectStoreTrait,
    dispatcher: &DispatcherClient,
    msg: crate::pgqueue::Message,
    requeue_delay: Duration,
    caps: &CryoArtifactCaps,
) -> anyhow::Result<()> {
    let message_id = msg.message_id.clone();
    let ack_token = msg.ack_token.clone();
    let wakeup: TaskWakeup = match serde_json::from_value(msg.payload.clone()) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(
                event = "harness.cryo_worker.wakeup.invalid",
                error = %err,
                message_id = %message_id,
                "invalid wakeup payload; dropping"
            );
            queue.ack(&ack_token).await?;
            return Ok(());
        }
    };

    let res: anyhow::Result<()> = async {
        let _ = run_task_with_caps(cfg, object_store, dispatcher, wakeup.task_id, caps).await?;
        queue.ack(&ack_token).await?;
        Ok(())
    }
    .await;

    match res {
        Ok(()) => Ok(()),
        Err(err) => {
            queue.nack_or_requeue(&ack_token, requeue_delay).await?;
            Err(err)
        }
    }
}

async fn write_dataset_artifacts(
    object_store: &dyn ObjectStoreTrait,
    pubd: &DatasetPublication,
    payload: &CryoIngestPayload,
    task_id: Uuid,
    attempt: i64,
    caps: &CryoArtifactCaps,
) -> Result<(), CryoArtifactError> {
    let staging_dir = staging_dir_for_task(task_id, attempt);
    ensure_private_dir(&staging_dir)
        .await
        .with_context(|| format!("create staging dir {}", staging_dir.display()))
        .map_err(CryoArtifactError::Retryable)?;

    let mode = std::env::var("TRACE_CRYO_MODE")
        .unwrap_or_else(|_| "fake".to_string())
        .to_ascii_lowercase();
    if mode == "real" {
        write_dataset_artifacts_real(object_store, pubd, payload, &staging_dir, caps).await?;
    } else {
        write_dataset_artifacts_fake(object_store, pubd, payload, &staging_dir).await?;
    }

    let _ = tokio::fs::remove_dir_all(&staging_dir).await;
    Ok(())
}

async fn write_dataset_artifacts_fake(
    object_store: &dyn ObjectStoreTrait,
    pubd: &DatasetPublication,
    payload: &CryoIngestPayload,
    staging_dir: &Path,
) -> Result<(), CryoArtifactError> {
    let (bucket, prefix_key) = match &pubd.storage_ref {
        DatasetStorageRef::S3 { bucket, prefix, .. } => (bucket.clone(), prefix.clone()),
        DatasetStorageRef::File { .. } => {
            return Err(CryoArtifactError::Fatal(anyhow::anyhow!(
                "cryo worker requires s3 storage ref"
            )));
        }
    };

    let file_name = format!("cryo_{}_{}.parquet", payload.range_start, payload.range_end);
    let parquet_key = join_key(&prefix_key, &file_name);
    let parquet_path = build_parquet_file(staging_dir, payload)
        .await
        .map_err(CryoArtifactError::Retryable)?;
    object_store
        .put_file(&bucket, &parquet_key, &parquet_path, CONTENT_TYPE_PARQUET)
        .await
        .context("upload parquet")
        .map_err(CryoArtifactError::Retryable)?;

    write_manifest(object_store, pubd, &bucket, &prefix_key, vec![parquet_key]).await?;

    Ok(())
}

fn join_key(prefix: &str, leaf: &str) -> String {
    let prefix = prefix.trim_end_matches('/');
    format!("{prefix}/{leaf}")
}

fn derive_dataset_version(payload: &CryoIngestPayload) -> Uuid {
    let name = format!(
        "cryo_ingest:{}:{}:{}:{}:{}",
        payload.dataset_uuid,
        payload.config_hash,
        payload.chain_id,
        payload.range_start,
        payload.range_end
    );
    Uuid::new_v5(&DATASET_VERSION_NAMESPACE, name.as_bytes())
}

async fn write_dataset_artifacts_real(
    object_store: &dyn ObjectStoreTrait,
    pubd: &DatasetPublication,
    payload: &CryoIngestPayload,
    staging_dir: &Path,
    caps: &CryoArtifactCaps,
) -> Result<(), CryoArtifactError> {
    let rpc_url = payload
        .rpc_pool
        .as_deref()
        .and_then(rpc_url_for_pool)
        .or_else(|| std::env::var("TRACE_CRYO_RPC_URL").ok())
        .ok_or_else(|| {
            CryoArtifactError::Fatal(anyhow::anyhow!(
                "missing RPC URL: set TRACE_CRYO_RPC_URL or TRACE_RPC_POOL_<NAME>_URL"
            ))
        })?;
    let cryo_bin = std::env::var("TRACE_CRYO_BIN").unwrap_or_else(|_| "cryo".to_string());

    let dataset_name = payload
        .cryo_dataset_name
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| dataset_name_from_config_hash(&payload.config_hash))
        .ok_or_else(|| {
            CryoArtifactError::Fatal(anyhow::anyhow!(
                "missing cryo_dataset_name and unrecognized config_hash for dataset name"
            ))
        })?;

    let (bucket, prefix_key) = match &pubd.storage_ref {
        DatasetStorageRef::S3 { bucket, prefix, .. } => (bucket.clone(), prefix.clone()),
        DatasetStorageRef::File { .. } => {
            return Err(CryoArtifactError::Fatal(anyhow::anyhow!(
                "cryo worker requires s3 storage ref"
            )));
        }
    };

    run_cryo_cli(
        &cryo_bin,
        dataset_name,
        &rpc_url,
        payload.range_start,
        payload.range_end,
        staging_dir,
    )
    .await?;

    let parquet_files = tokio::task::spawn_blocking({
        let out_dir = staging_dir.to_path_buf();
        move || collect_parquet_files(&out_dir)
    })
    .await
    .map_err(|err| CryoArtifactError::Retryable(anyhow::anyhow!(err)))?
    .map_err(CryoArtifactError::Retryable)?;

    if parquet_files.is_empty() {
        return Err(CryoArtifactError::Fatal(anyhow::anyhow!(
            "cryo produced no parquet files"
        )));
    }

    enforce_parquet_caps(&parquet_files, caps)?;

    let mut parquet_keys = Vec::with_capacity(parquet_files.len());
    for file in parquet_files {
        let file_path = file.path;
        let rel = file_path
            .strip_prefix(staging_dir)
            .unwrap_or(file_path.as_path());
        let rel = rel.to_string_lossy().replace('\\', "/");
        let key = join_key(&prefix_key, &rel);

        object_store
            .put_file(&bucket, &key, &file_path, CONTENT_TYPE_PARQUET)
            .await
            .with_context(|| format!("upload parquet object {key}"))
            .map_err(CryoArtifactError::Retryable)?;

        parquet_keys.push(key);
    }

    write_manifest(object_store, pubd, &bucket, &prefix_key, parquet_keys).await?;

    Ok(())
}

#[derive(Debug)]
struct ParquetFile {
    path: PathBuf,
    size_bytes: u64,
}

fn enforce_parquet_caps(
    parquet_files: &[ParquetFile],
    caps: &CryoArtifactCaps,
) -> Result<(), CryoArtifactError> {
    if parquet_files.len() > caps.max_parquet_files_per_range {
        return Err(CryoArtifactError::Fatal(anyhow::anyhow!(
            "parquet file count exceeds MAX_PARQUET_FILES_PER_RANGE: got={} max={}",
            parquet_files.len(),
            caps.max_parquet_files_per_range
        )));
    }

    let mut total_bytes = 0_u64;
    for file in parquet_files {
        if file.size_bytes > caps.max_parquet_bytes_per_file {
            return Err(CryoArtifactError::Fatal(anyhow::anyhow!(
                "parquet file exceeds MAX_PARQUET_BYTES_PER_FILE: path={} size_bytes={} max={}",
                file.path.display(),
                file.size_bytes,
                caps.max_parquet_bytes_per_file
            )));
        }

        total_bytes = total_bytes.saturating_add(file.size_bytes);
        if total_bytes > caps.max_total_parquet_bytes_per_range {
            return Err(CryoArtifactError::Fatal(anyhow::anyhow!(
                "total parquet bytes exceeds MAX_TOTAL_PARQUET_BYTES_PER_RANGE: total_bytes={} max={}",
                total_bytes,
                caps.max_total_parquet_bytes_per_range
            )));
        }
    }

    Ok(())
}

fn dataset_name_from_config_hash(config_hash: &str) -> Option<&str> {
    // Expected harness format: `cryo_ingest.<dataset>:<version>`
    let without_prefix = config_hash.strip_prefix("cryo_ingest.")?;
    Some(without_prefix.split(':').next()?)
}

fn rpc_url_for_pool(pool: &str) -> Option<String> {
    let pool = pool.trim();
    if pool.is_empty() {
        return None;
    }

    let mut key = String::with_capacity(pool.len());
    for c in pool.chars() {
        if c.is_ascii_alphanumeric() {
            key.push(c.to_ascii_uppercase());
        } else {
            key.push('_');
        }
    }

    let env_key = format!("TRACE_RPC_POOL_{key}_URL");
    std::env::var(env_key).ok()
}

async fn run_cryo_cli(
    cryo_bin: &str,
    dataset: &str,
    rpc_url: &str,
    start_block: i64,
    end_block: i64,
    output_dir: &Path,
) -> Result<(), CryoArtifactError> {
    // Cryo's --blocks start:end syntax is end-exclusive, matching our range convention.
    let out = tokio::task::spawn_blocking({
        let cryo_bin = cryo_bin.to_string();
        let dataset = dataset.to_string();
        let rpc_url = rpc_url.to_string();
        let output_dir = output_dir.to_path_buf();
        move || {
            std::fs::create_dir_all(&output_dir).map_err(|err| {
                CryoArtifactError::Fatal(anyhow::Error::new(err).context("create output dir"))
            })?;

            std::process::Command::new(&cryo_bin)
                .arg(dataset)
                .arg("--rpc")
                .arg(rpc_url)
                .arg("--blocks")
                .arg(format!("{}:{}", start_block, end_block))
                .arg("--output-dir")
                .arg(output_dir.to_string_lossy().to_string())
                .output()
                .map_err(|err| {
                    let kind = err.kind();
                    let wrapped = anyhow::Error::new(err).context("run cryo");
                    if kind == std::io::ErrorKind::NotFound {
                        CryoArtifactError::Fatal(wrapped)
                    } else {
                        CryoArtifactError::Retryable(wrapped)
                    }
                })
        }
    })
    .await
    .map_err(|err| CryoArtifactError::Retryable(anyhow::anyhow!(err)))??;

    if out.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stderr = stderr.trim();
    let stderr = if stderr.len() > 1024 {
        &stderr[..1024]
    } else {
        stderr
    };

    let msg = if stderr.is_empty() {
        "cryo failed"
    } else {
        stderr
    };

    let err = anyhow::anyhow!("{msg}");
    match out.status.code() {
        Some(2) => Err(CryoArtifactError::Fatal(err)),
        _ => Err(CryoArtifactError::Retryable(err)),
    }
}

fn collect_parquet_files(dir: &Path) -> anyhow::Result<Vec<ParquetFile>> {
    let mut out = Vec::new();
    collect_parquet_files_into(dir, &mut out)?;
    Ok(out)
}

fn collect_parquet_files_into(dir: &Path, out: &mut Vec<ParquetFile>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let entry = entry.context("read_dir entry")?;
        let path = entry.path();
        let file_type = entry.file_type().context("file_type")?;

        if file_type.is_dir() {
            if path.file_name().is_some_and(|name| name == ".cryo") {
                continue;
            }
            collect_parquet_files_into(&path, out)?;
            continue;
        }

        if file_type.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("parquet"))
        {
            let meta =
                std::fs::metadata(&path).with_context(|| format!("metadata {}", path.display()))?;
            out.push(ParquetFile {
                path,
                size_bytes: meta.len(),
            });
        }
    }
    Ok(())
}

fn staging_root() -> PathBuf {
    PathBuf::from("/tmp/trace/cryo")
}

fn staging_dir_for_task(task_id: Uuid, attempt: i64) -> PathBuf {
    staging_root()
        .join(task_id.to_string())
        .join(attempt.to_string())
}

async fn ensure_private_dir(path: &Path) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(path)
        .await
        .with_context(|| format!("create dir {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .await
            .with_context(|| format!("set permissions {}", path.display()))?;
    }

    Ok(())
}

async fn cleanup_stale_staging_dirs(root: PathBuf, ttl: Duration) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || {
        if !root.exists() {
            return Ok(());
        }

        let now = SystemTime::now();
        for task_entry in std::fs::read_dir(&root).context("read staging root")? {
            let task_entry = task_entry.context("read staging root entry")?;
            let task_path = task_entry.path();
            if !task_entry
                .file_type()
                .context("staging file_type")?
                .is_dir()
            {
                continue;
            }

            for attempt_entry in std::fs::read_dir(&task_path)
                .with_context(|| format!("read task dir {}", task_path.display()))?
            {
                let attempt_entry = attempt_entry.context("read attempt entry")?;
                let attempt_path = attempt_entry.path();
                if !attempt_entry
                    .file_type()
                    .context("attempt file_type")?
                    .is_dir()
                {
                    continue;
                }

                let meta = std::fs::metadata(&attempt_path)
                    .with_context(|| format!("metadata {}", attempt_path.display()))?;
                let modified = meta.modified().unwrap_or(now);
                let age = now.duration_since(modified).unwrap_or_default();
                if age > ttl {
                    let _ = std::fs::remove_dir_all(&attempt_path);
                }
            }
        }

        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("join cleanup task")?
}

async fn build_parquet_file(
    staging_dir: &Path,
    payload: &CryoIngestPayload,
) -> anyhow::Result<PathBuf> {
    let payload = payload.clone();
    let staging_dir = staging_dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&staging_dir).context("create staging dir")?;
        let parquet_path = staging_dir.join("data.parquet");

        let last_block = payload.range_end.saturating_sub(1).max(payload.range_start);

        let mut blocks = BTreeSet::new();
        blocks.insert(payload.range_start);
        blocks.insert(last_block);
        blocks.insert(payload.range_start.saturating_add(1).min(last_block));

        let conn = Connection::open_in_memory().context("open duckdb in-memory")?;
        conn.execute_batch(
            r#"
            BEGIN;
            CREATE TABLE blocks (
              chain_id BIGINT NOT NULL,
              block_number BIGINT NOT NULL,
              block_hash VARCHAR NOT NULL
            );
            COMMIT;
            "#,
        )
        .context("create blocks table")?;

        for block_number in blocks {
            let block_hash = format!("0x{block_number:016x}");
            conn.execute(
                "INSERT INTO blocks VALUES (?, ?, ?)",
                duckdb::params![payload.chain_id, block_number, block_hash],
            )
            .context("insert block row")?;
        }

        let parquet_escaped = parquet_path.to_string_lossy().replace('\'', "''");
        conn.execute_batch(&format!(
            "COPY blocks TO '{parquet_escaped}' (FORMAT PARQUET);"
        ))
        .context("copy to parquet")?;

        Ok::<_, anyhow::Error>(parquet_path)
    })
    .await
    .context("join parquet builder")?
}

async fn write_manifest(
    object_store: &dyn ObjectStoreTrait,
    pubd: &DatasetPublication,
    bucket: &str,
    prefix_key: &str,
    mut parquet_keys: Vec<String>,
) -> Result<(), CryoArtifactError> {
    parquet_keys.sort();
    parquet_keys.dedup();

    let manifest = DatasetManifestV1 {
        version: DatasetManifestV1::VERSION,
        dataset_uuid: pubd.dataset_uuid,
        dataset_version: pubd.dataset_version,
        parquet_keys,
    };
    let bytes = serde_json::to_vec(&manifest)
        .context("encode manifest json")
        .map_err(CryoArtifactError::Retryable)?;

    let manifest_key = join_key(prefix_key, "_manifest.json");
    object_store
        .put_bytes(bucket, &manifest_key, bytes, CONTENT_TYPE_JSON)
        .await
        .with_context(|| format!("upload manifest object {manifest_key}"))
        .map_err(CryoArtifactError::Retryable)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use trace_core::{Error, Result as CoreResult};

    #[derive(Default, Clone)]
    struct RecordingObjectStore {
        puts: Arc<Mutex<Vec<PutOp>>>,
    }

    #[derive(Debug, Clone)]
    enum PutOp {
        Bytes { key: String, content_type: String },
        File { key: String },
    }

    impl RecordingObjectStore {
        fn ops(&self) -> Vec<PutOp> {
            self.puts
                .lock()
                .expect("mutex poisoned")
                .iter()
                .cloned()
                .collect()
        }
    }

    #[async_trait]
    impl ObjectStoreTrait for RecordingObjectStore {
        async fn put_bytes(
            &self,
            _bucket: &str,
            key: &str,
            _bytes: Vec<u8>,
            content_type: &str,
        ) -> CoreResult<()> {
            if content_type == CONTENT_TYPE_PARQUET {
                return Err(Error::msg(
                    "regression: parquet uploads must use put_file (streaming), not put_bytes",
                ));
            }
            self.puts
                .lock()
                .expect("mutex poisoned")
                .push(PutOp::Bytes {
                    key: key.to_string(),
                    content_type: content_type.to_string(),
                });
            Ok(())
        }

        async fn put_file(
            &self,
            _bucket: &str,
            key: &str,
            local_path: &Path,
            content_type: &str,
        ) -> CoreResult<()> {
            if content_type != CONTENT_TYPE_PARQUET {
                return Err(Error::msg(format!(
                    "unexpected put_file content_type={content_type} path={}",
                    local_path.display()
                )));
            }
            if !local_path.exists() {
                return Err(Error::msg(format!(
                    "put_file local_path missing: {}",
                    local_path.display()
                )));
            }
            self.puts.lock().expect("mutex poisoned").push(PutOp::File {
                key: key.to_string(),
            });
            Ok(())
        }

        async fn get_bytes(&self, _bucket: &str, _key: &str) -> CoreResult<Vec<u8>> {
            Err(Error::msg("unexpected get_bytes in cryo worker test"))
        }
    }

    #[tokio::test]
    async fn fake_artifact_upload_uses_put_file_for_parquet() -> anyhow::Result<()> {
        let object_store = RecordingObjectStore::default();
        let payload = CryoIngestPayload {
            dataset_uuid: Uuid::new_v4(),
            chain_id: 1,
            range_start: 0,
            range_end: 10,
            config_hash: "cryo_ingest.blocks:1".to_string(),
            dataset_key: Some("blocks".to_string()),
            cryo_dataset_name: Some("blocks".to_string()),
            rpc_pool: None,
        };
        let pubd = derive_dataset_publication("test-bucket", &payload);

        let staging_dir = std::env::temp_dir().join(format!("trace-cryo-test-{}", Uuid::new_v4()));
        ensure_private_dir(&staging_dir).await?;
        let res = write_dataset_artifacts_fake(&object_store, &pubd, &payload, &staging_dir).await;
        let _ = tokio::fs::remove_dir_all(&staging_dir).await;
        res.map_err(|err| anyhow::anyhow!("{err:?}"))?;

        let ops = object_store.ops();
        for op in &ops {
            match op {
                PutOp::Bytes { key, content_type } => {
                    assert!(
                        content_type == CONTENT_TYPE_JSON,
                        "unexpected content type for put_bytes: {content_type}"
                    );
                    assert!(
                        key.ends_with("/_manifest.json"),
                        "expected manifest key, got {key}"
                    );
                }
                PutOp::File { key } => {
                    assert!(key.ends_with(".parquet"), "expected parquet key, got {key}");
                }
            }
        }
        let file_puts = ops
            .iter()
            .filter(|op| matches!(op, PutOp::File { .. }))
            .count();
        let json_puts = ops
            .iter()
            .filter(|op| matches!(op, PutOp::Bytes { content_type, .. } if content_type == CONTENT_TYPE_JSON))
            .count();

        assert_eq!(
            file_puts, 1,
            "expected one parquet put_file call: ops={ops:?}"
        );
        assert_eq!(
            json_puts, 1,
            "expected one manifest put_bytes call: ops={ops:?}"
        );

        Ok(())
    }

    #[test]
    fn parquet_caps_exceeded_is_fatal() {
        let caps = CryoArtifactCaps {
            max_parquet_files_per_range: 1,
            max_parquet_bytes_per_file: 10,
            max_total_parquet_bytes_per_range: 20,
        };

        let files = vec![
            ParquetFile {
                path: PathBuf::from("a.parquet"),
                size_bytes: 1,
            },
            ParquetFile {
                path: PathBuf::from("b.parquet"),
                size_bytes: 1,
            },
        ];

        match enforce_parquet_caps(&files, &caps) {
            Err(CryoArtifactError::Fatal(_)) => {}
            other => panic!("expected fatal error, got {other:?}"),
        }

        let files = vec![ParquetFile {
            path: PathBuf::from("big.parquet"),
            size_bytes: 11,
        }];

        match enforce_parquet_caps(&files, &caps) {
            Err(CryoArtifactError::Fatal(_)) => {}
            other => panic!("expected fatal error, got {other:?}"),
        }

        let files = vec![
            ParquetFile {
                path: PathBuf::from("a.parquet"),
                size_bytes: 10,
            },
            ParquetFile {
                path: PathBuf::from("b.parquet"),
                size_bytes: 11,
            },
        ];

        match enforce_parquet_caps(&files, &caps) {
            Err(CryoArtifactError::Fatal(_)) => {}
            other => panic!("expected fatal error, got {other:?}"),
        }
    }
}
