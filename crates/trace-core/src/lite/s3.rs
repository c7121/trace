use crate::{Error, ObjectStore as ObjectStoreTrait, Result};
use anyhow::Context;
use async_trait::async_trait;
use reqwest::Url;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ObjectStore {
    endpoint: Url,
    client: Arc<reqwest::Client>,
}

impl ObjectStore {
    pub fn new(endpoint: &str) -> Result<Self> {
        Ok(Self {
            endpoint: endpoint
                .parse()
                .context("parse S3 endpoint URL")
                .map_err(Error::from)?,
            client: Arc::new(reqwest::Client::new()),
        })
    }

    pub async fn put_bytes(
        &self,
        bucket: &str,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<()> {
        let url = object_url(&self.endpoint, bucket, key)?;
        let resp = self
            .client
            .put(url)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(bytes)
            .send()
            .await
            .context("PUT object")
            .map_err(Error::from)?;

        let resp = resp
            .error_for_status()
            .context("PUT object status")
            .map_err(Error::from)?;
        drop(resp);
        Ok(())
    }

    pub async fn get_bytes(&self, bucket: &str, key: &str) -> Result<Vec<u8>> {
        let url = object_url(&self.endpoint, bucket, key)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .context("GET object")
            .map_err(Error::from)?;
        let resp = resp
            .error_for_status()
            .context("GET object status")
            .map_err(Error::from)?;
        Ok(resp
            .bytes()
            .await
            .context("GET body bytes")
            .map_err(Error::from)?
            .to_vec())
    }
}

#[async_trait]
impl ObjectStoreTrait for ObjectStore {
    async fn put_bytes(
        &self,
        bucket: &str,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<()> {
        self.put_bytes(bucket, key, bytes, content_type).await
    }

    async fn get_bytes(&self, bucket: &str, key: &str) -> Result<Vec<u8>> {
        self.get_bytes(bucket, key).await
    }
}

pub fn parse_s3_uri(uri: &str) -> Result<(String, String)> {
    let uri = uri
        .strip_prefix("s3://")
        .ok_or_else(|| Error::msg("batch_uri must start with s3://"))?;
    let (bucket, key) = uri
        .split_once('/')
        .ok_or_else(|| Error::msg("s3 uri missing key"))?;
    Ok((bucket.to_string(), key.to_string()))
}

fn object_url(endpoint: &Url, bucket: &str, key: &str) -> Result<Url> {
    let base = endpoint.as_str().trim_end_matches('/');
    let full = format!("{base}/{bucket}/{key}");
    full.parse()
        .context("build object URL")
        .map_err(Error::from)
}
