use crate::config::QueryServiceConfig;
use anyhow::Context;
use reqwest::StatusCode;
use serde::Deserialize;
use trace_core::lite::s3::parse_s3_uri;
use trace_core::ObjectStore as ObjectStoreTrait;

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

/// Parquet dataset resolved from a pinned manifest.
///
/// This is intentionally *not* downloaded to local disk. Query Service attaches the dataset in
/// DuckDB using remote URLs so DuckDB can do Parquet pushdown (projection + predicate) with
/// HTTP range reads.
pub struct RemoteParquetDataset {
    pub parquet_urls: Vec<String>,
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

pub async fn resolve_parquet_urls(
    s3_endpoint: &str,
    http_client: &reqwest::Client,
    parquet_objects: &[String],
    limits: &DatasetDownloadLimits,
) -> Result<RemoteParquetDataset, DatasetLoadError> {
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

    // Preflight object sizes via HEAD so we can enforce hard caps without downloading the data.
    //
    // NOTE: DuckDB will still read Parquet data during query execution (using HTTP range reads).
    // These caps protect the service from accidentally attaching huge datasets.
    let mut remaining_budget = limits.max_total_bytes;
    for uri in parquet_objects {
        if uri.len() > limits.max_uri_len {
            return Err(DatasetLoadError::Invalid(anyhow::anyhow!(
                "parquet uri exceeds max_uri_len"
            )));
        }

        let (bucket, key) = parse_s3_uri(uri)
            .context("parse parquet s3 uri")
            .map_err(DatasetLoadError::Invalid)?;

        head_object_size(
            s3_endpoint,
            http_client,
            &bucket,
            &key,
            limits.max_object_bytes,
            &mut remaining_budget,
        )
        .await?;
    }

    // Convert s3://bucket/key URIs to HTTP URLs served by the configured endpoint.
    // In Lite/harness the MinIO bucket is anonymous/public.
    let mut parquet_urls = Vec::with_capacity(parquet_objects.len());
    for uri in parquet_objects {
        let (bucket, key) = parse_s3_uri(uri)
            .context("parse parquet s3 uri")
            .map_err(DatasetLoadError::Invalid)?;
        let url = object_url(s3_endpoint, &bucket, &key)
            .map_err(DatasetLoadError::Invalid)?;
        parquet_urls.push(url.to_string());
    }

    Ok(RemoteParquetDataset { parquet_urls })
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

async fn head_object_size(
    s3_endpoint: &str,
    http_client: &reqwest::Client,
    bucket: &str,
    key: &str,
    max_object_bytes: u64,
    remaining_budget: &mut u64,
) -> Result<u64, DatasetLoadError> {
    let url = object_url(s3_endpoint, bucket, key).map_err(DatasetLoadError::Invalid)?;

    let resp = http_client
        .head(url.clone())
        .send()
        .await
        .with_context(|| format!("HEAD {url}"))
        .map_err(DatasetLoadError::Retryable)?;

    let status = resp.status();
    if !status.is_success() {
        let err = anyhow::anyhow!("HEAD {url} returned {status}");
        if status == StatusCode::NOT_FOUND || status.is_server_error() {
            return Err(DatasetLoadError::Retryable(err));
        }
        return Err(DatasetLoadError::Invalid(err));
    }

    let Some(content_len) = resp.content_length() else {
        // Fail closed: without Content-Length we cannot safely enforce hard limits.
        return Err(DatasetLoadError::Invalid(anyhow::anyhow!(
            "object missing Content-Length"
        )));
    };

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

    *remaining_budget = remaining_budget.saturating_sub(content_len);
    Ok(content_len)
}

fn object_url(endpoint: &str, bucket: &str, key: &str) -> anyhow::Result<reqwest::Url> {
    let base = endpoint.trim_end_matches('/');
    let full = format!("{base}/{bucket}/{key}");
    Ok(full.parse().context("build object url")?)
}
