use crate::s3::parse_s3_uri;
use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use trace_core::{udf::UdfInvocationPayload, ObjectStore as ObjectStoreTrait};
use uuid::Uuid;

use crate::constants::CONTENT_TYPE_JSONL;
use crate::dispatcher_client::{
    BufferPublishRequest, CompleteRequest, DispatcherClient, WriteDisposition,
};

fn default_payload() -> Value {
    Value::Object(Map::new())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FakeBundle {
    alert_definition_id: Uuid,
    dedupe_key: String,
    chain_id: i64,
    block_number: i64,
    block_hash: String,
    tx_hash: String,

    #[serde(default = "default_payload")]
    payload: Value,
}

#[derive(Debug, Serialize)]
struct AlertEventRow {
    alert_definition_id: Uuid,
    dedupe_key: String,
    event_time: DateTime<Utc>,
    chain_id: i64,
    block_number: i64,
    block_hash: String,
    tx_hash: String,
    payload: Value,
}

#[derive(Clone)]
pub struct FakeRunner {
    dispatcher: DispatcherClient,
    bucket: String,
    object_store: std::sync::Arc<dyn ObjectStoreTrait>,
    http: reqwest::Client,
}

impl FakeRunner {
    pub fn new(
        dispatcher_url: String,
        bucket: String,
        object_store: std::sync::Arc<dyn ObjectStoreTrait>,
    ) -> Self {
        Self {
            dispatcher: DispatcherClient::new(dispatcher_url),
            bucket,
            object_store,
            http: reqwest::Client::new(),
        }
    }

    pub async fn run(&self, invocation: &UdfInvocationPayload) -> anyhow::Result<()> {
        let bundle_bytes = self
            .fetch_bundle_bytes(&invocation.bundle_url)
            .await
            .context("fetch bundle bytes")?;
        let bundle: FakeBundle =
            serde_json::from_slice(&bundle_bytes).context("decode fake bundle json")?;

        if !bundle.payload.is_object() {
            return Err(anyhow!("bundle payload must be a JSON object"));
        }

        let row = AlertEventRow {
            alert_definition_id: bundle.alert_definition_id,
            dedupe_key: bundle.dedupe_key,
            event_time: Utc::now(),
            chain_id: bundle.chain_id,
            block_number: bundle.block_number,
            block_hash: bundle.block_hash,
            tx_hash: bundle.tx_hash,
            payload: bundle.payload,
        };

        let key = format!(
            "batches/{}/{}/udf.jsonl",
            invocation.task_id, invocation.attempt
        );
        let batch_uri = format!("s3://{}/{}", self.bucket, key);
        let mut bytes = serde_json::to_vec(&row).context("encode alert event row")?;
        bytes.push(b'\n');

        self.object_store
            .put_bytes(&self.bucket, &key, bytes.clone(), CONTENT_TYPE_JSONL)
            .await
            .context("upload batch")?;

        self.buffer_publish(invocation, &batch_uri, bytes.len())
            .await
            .context("buffer publish")?;

        self.complete(invocation).await.context("complete")?;
        Ok(())
    }

    async fn fetch_bundle_bytes(&self, url: &str) -> anyhow::Result<Vec<u8>> {
        if url.starts_with("s3://") {
            let (bucket, key) = parse_s3_uri(url)?;
            return Ok(self.object_store.get_bytes(&bucket, &key).await?);
        }

        let resp = self
            .http
            .get(url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        let resp = resp.error_for_status().context("GET bundle status")?;
        Ok(resp.bytes().await.context("GET bundle bytes")?.to_vec())
    }

    async fn buffer_publish(
        &self,
        invocation: &UdfInvocationPayload,
        batch_uri: &str,
        batch_size_bytes: usize,
    ) -> anyhow::Result<()> {
        let req = BufferPublishRequest {
            task_id: invocation.task_id,
            attempt: invocation.attempt,
            lease_token: invocation.lease_token,
            batch_uri: batch_uri.to_string(),
            content_type: CONTENT_TYPE_JSONL.to_string(),
            batch_size_bytes: batch_size_bytes.min(i64::MAX as usize) as i64,
            dedupe_scope: "udf".to_string(),
        };

        if self
            .dispatcher
            .buffer_publish(&invocation.capability_token, &req)
            .await?
            == WriteDisposition::Conflict
        {
            return Ok(());
        }

        Ok(())
    }

    async fn complete(&self, invocation: &UdfInvocationPayload) -> anyhow::Result<()> {
        let req = CompleteRequest {
            task_id: invocation.task_id,
            attempt: invocation.attempt,
            lease_token: invocation.lease_token,
            outcome: "success",
        };

        if self
            .dispatcher
            .complete(&invocation.capability_token, &req)
            .await?
            == WriteDisposition::Conflict
        {
            return Ok(());
        }

        Ok(())
    }
}
