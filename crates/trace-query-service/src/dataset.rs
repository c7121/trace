use crate::config::QueryServiceConfig;
use anyhow::Context;
use serde::Deserialize;
use trace_core::lite::s3::parse_s3_uri;
use trace_core::ObjectStore as ObjectStoreTrait;
use uuid::Uuid;
use std::path::PathBuf;

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

/// Parquet dataset downloaded to a local temp directory.
///
/// The temp directory is deleted on drop.
pub struct DownloadedParquetDataset {
    dir: PathBuf,
    pub parquet_paths: Vec<PathBuf>,
}

impl Drop for DownloadedParquetDataset {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
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

pub async fn download_parquet_objects(
    object_store: &dyn ObjectStoreTrait,
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

    let dir = std::env::temp_dir().join(format!("trace-query-dataset-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&dir)
        .await
        .context("create dataset temp dir")
        .map_err(DatasetLoadError::Retryable)?;

    let mut remaining_budget = limits.max_total_bytes;
    let mut parquet_paths = Vec::with_capacity(parquet_objects.len());
    for (idx, uri) in parquet_objects.iter().enumerate() {
        if uri.len() > limits.max_uri_len {
            return Err(DatasetLoadError::Invalid(anyhow::anyhow!(
                "parquet uri exceeds max_uri_len"
            )));
        }

        let (bucket, key) = parse_s3_uri(uri)
            .context("parse parquet s3 uri")
            .map_err(DatasetLoadError::Invalid)?;

        let bytes = object_store
            .get_bytes(&bucket, &key)
            .await
            .context("fetch parquet object")
            .map_err(DatasetLoadError::Retryable)?;

        let object_len = bytes.len() as u64;
        if object_len > limits.max_object_bytes {
            return Err(DatasetLoadError::TooLarge(anyhow::anyhow!(
                "object exceeds max_object_bytes: {object_len} > {}",
                limits.max_object_bytes
            )));
        }
        if object_len > remaining_budget {
            return Err(DatasetLoadError::TooLarge(anyhow::anyhow!(
                "dataset exceeds max_total_bytes: {object_len} > {remaining_budget}"
            )));
        }
        remaining_budget = remaining_budget.saturating_sub(object_len);

        let path = dir.join(format!("{idx}.parquet"));
        tokio::fs::write(&path, bytes)
            .await
            .context("write parquet object")
            .map_err(DatasetLoadError::Retryable)?;
        parquet_paths.push(path);
    }

    Ok(DownloadedParquetDataset { dir, parquet_paths })
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
