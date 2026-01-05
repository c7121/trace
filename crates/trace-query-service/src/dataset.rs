use anyhow::Context;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use trace_core::lite::s3::parse_s3_uri;
use trace_core::ObjectStore as ObjectStoreTrait;
use uuid::Uuid;

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
) -> anyhow::Result<DatasetManifest> {
    let uri = manifest_uri(storage_prefix);
    let (bucket, key) = parse_s3_uri(&uri).context("parse manifest s3 uri")?;
    let bytes = object_store
        .get_bytes(&bucket, &key)
        .await
        .context("fetch dataset manifest")?;

    let manifest: DatasetManifest =
        serde_json::from_slice(&bytes).context("parse manifest json")?;
    Ok(manifest)
}

pub struct DownloadedParquetDataset {
    _dir: TempDir,
    pub parquet_glob: PathBuf,
}

pub async fn download_parquet_objects(
    object_store: &dyn ObjectStoreTrait,
    parquet_objects: &[String],
) -> anyhow::Result<DownloadedParquetDataset> {
    let dir = TempDir::new("trace-query-service-parquet")?;

    for (idx, uri) in parquet_objects.iter().enumerate() {
        let (bucket, key) = parse_s3_uri(uri).context("parse parquet s3 uri")?;
        let bytes = object_store
            .get_bytes(&bucket, &key)
            .await
            .context("fetch parquet object")?;

        let file_name = format!("part-{idx:03}.parquet");
        let path = dir.path().join(file_name);
        tokio::fs::write(&path, bytes)
            .await
            .with_context(|| format!("write parquet bytes to {}", path.display()))?;
    }

    Ok(DownloadedParquetDataset {
        parquet_glob: dir.path().join("*.parquet"),
        _dir: dir,
    })
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
