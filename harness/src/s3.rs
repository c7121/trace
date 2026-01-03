use anyhow::{anyhow, Context};
use reqwest::Url;
use std::sync::Arc;

#[derive(Clone)]
pub struct ObjectStore {
    endpoint: Url,
    client: Arc<reqwest::Client>,
}

impl ObjectStore {
    pub fn new(endpoint: &str) -> anyhow::Result<Self> {
        Ok(Self {
            endpoint: endpoint.parse().context("parse S3 endpoint URL")?,
            client: Arc::new(reqwest::Client::new()),
        })
    }

    pub async fn put_bytes(
        &self,
        bucket: &str,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> anyhow::Result<()> {
        let url = object_url(&self.endpoint, bucket, key)?;
        let resp = self
            .client
            .put(url)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(bytes)
            .send()
            .await
            .context("PUT object")?;

        let resp = resp.error_for_status().context("PUT object status")?;
        drop(resp);
        Ok(())
    }

    pub async fn get_bytes(&self, bucket: &str, key: &str) -> anyhow::Result<Vec<u8>> {
        let url = object_url(&self.endpoint, bucket, key)?;
        let resp = self.client.get(url).send().await.context("GET object")?;
        let resp = resp.error_for_status().context("GET object status")?;
        Ok(resp.bytes().await.context("GET body bytes")?.to_vec())
    }
}

pub fn parse_s3_uri(uri: &str) -> anyhow::Result<(String, String)> {
    let uri = uri
        .strip_prefix("s3://")
        .ok_or_else(|| anyhow!("batch_uri must start with s3://"))?;
    let (bucket, key) = uri
        .split_once('/')
        .ok_or_else(|| anyhow!("s3 uri missing key"))?;
    Ok((bucket.to_string(), key.to_string()))
}

fn object_url(endpoint: &Url, bucket: &str, key: &str) -> anyhow::Result<Url> {
    let base = endpoint.as_str().trim_end_matches('/');
    let full = format!("{base}/{bucket}/{key}");
    full.parse().context("build object URL")
}
