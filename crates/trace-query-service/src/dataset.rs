use crate::config::QueryServiceConfig;
use anyhow::Context;
use reqwest::Url;
use serde::Deserialize;
use trace_core::lite::s3::parse_s3_uri;
use trace_core::ObjectStore as ObjectStoreTrait;

/// Dataset attach limits.
///
/// The `_manifest.json` document and referenced objects are treated as **untrusted input**.
/// These limits are defensive (prevent unbounded disk/memory/CPU consumption).
#[derive(Debug, Clone)]
pub struct DatasetAttachLimits {
    pub max_manifest_bytes: usize,
    pub max_objects: usize,
    pub max_object_bytes: u64,
    pub max_total_bytes: u64,
    pub max_uri_len: usize,
}

impl From<&QueryServiceConfig> for DatasetAttachLimits {
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
#[serde(deny_unknown_fields)]
pub struct DatasetManifest {
    #[serde(default, alias = "objects")]
    pub parquet_objects: Vec<String>,
}

pub fn manifest_uri(storage_prefix: &str) -> String {
    let prefix = storage_prefix.trim_end_matches('/');
    format!("{prefix}/_manifest.json")
}

pub async fn fetch_manifest_only(
    object_store: &dyn ObjectStoreTrait,
    storage_prefix: &str,
    limits: &DatasetAttachLimits,
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

pub async fn s3_parquet_uris_to_http_urls(
    http: &reqwest::Client,
    s3_endpoint: &str,
    parquet_objects: &[String],
    limits: &DatasetAttachLimits,
) -> Result<Vec<String>, DatasetLoadError> {
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

    let mut remaining_budget = limits.max_total_bytes;
    let mut parquet_paths = Vec::with_capacity(parquet_objects.len());
    let base = parse_endpoint_base(s3_endpoint)?;

    for uri in parquet_objects {
        if uri.len() > limits.max_uri_len {
            return Err(DatasetLoadError::Invalid(anyhow::anyhow!(
                "parquet uri exceeds max_uri_len"
            )));
        }

        let (bucket, key) = parse_s3_uri(uri)
            .context("parse parquet s3 uri")
            .map_err(DatasetLoadError::Invalid)?;

        let url = http_object_url(&base, &bucket, &key)?;
        let object_len = head_content_length(http, &url).await?;

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

        if url.len() > limits.max_uri_len {
            return Err(DatasetLoadError::Invalid(anyhow::anyhow!(
                "parquet url exceeds max_uri_len"
            )));
        }
        parquet_paths.push(url);
    }

    Ok(parquet_paths)
}

fn parse_endpoint_base(endpoint: &str) -> Result<String, DatasetLoadError> {
    let endpoint: Url = endpoint
        .parse()
        .context("parse s3 endpoint url")
        .map_err(DatasetLoadError::Invalid)?;

    Ok(endpoint.as_str().trim_end_matches('/').to_string())
}

fn http_object_url(base: &str, bucket: &str, key: &str) -> Result<String, DatasetLoadError> {
    let url = format!("{base}/{bucket}/{key}");
    url.parse::<Url>()
        .context("build http object url")
        .map_err(DatasetLoadError::Invalid)?;

    Ok(url)
}

async fn head_content_length(
    http: &reqwest::Client,
    url: &str,
) -> Result<u64, DatasetLoadError> {
    let resp = http
        .head(url)
        .send()
        .await
        .context("HEAD parquet object")
        .map_err(DatasetLoadError::Retryable)?;

    let resp = resp
        .error_for_status()
        .context("HEAD parquet object status")
        .map_err(DatasetLoadError::Retryable)?;

    let len = resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or_else(|| {
            DatasetLoadError::Invalid(anyhow::anyhow!(
                "parquet object missing Content-Length"
            ))
        })?;

    Ok(len)
}

fn validate_manifest(
    manifest: &DatasetManifest,
    limits: &DatasetAttachLimits,
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
