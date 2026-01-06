use crate::s3::parse_s3_uri;
use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use trace_core::{udf::UdfInvocationPayload, ObjectStore as ObjectStoreTrait};
use uuid::Uuid;

use crate::constants::{CONTENT_TYPE_JSONL, TASK_CAPABILITY_HEADER};
use crate::dispatcher_client::{
    BufferPublishRequest, CompleteRequest, DispatcherClient, WriteDisposition,
};

use async_trait::async_trait;
use trace_core::runtime::RuntimeInvoker;

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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AlertEvaluateSpec {
    alert_definition_id: Uuid,
    dataset_id: Uuid,
    dataset_version: Uuid,
    sql: String,

    #[serde(default)]
    limit: Option<i64>,

    /// If set, attempt 1 completes as `retryable_error` after publishing outputs.
    #[serde(default)]
    retry_once: bool,

    /// If set, emit malformed JSONL output and fail the task as `fatal_error`.
    #[serde(default)]
    emit_malformed_output: bool,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct AlertEventRowWire {
    alert_definition_id: Uuid,
    dedupe_key: String,
    event_time: DateTime<Utc>,
    chain_id: i64,
    block_number: i64,
    block_hash: String,
    tx_hash: String,
    payload: Value,
}

#[derive(Debug, Serialize)]
struct TaskQueryRequestWire {
    task_id: Uuid,
    attempt: i64,
    dataset_id: Uuid,
    sql: String,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct TaskQueryResponseWire {
    rows: Vec<Vec<Value>>,
}

#[derive(Clone)]
pub struct FakeRunner {
    dispatcher: DispatcherClient,
    query_service_url: String,
    bucket: String,
    object_store: std::sync::Arc<dyn ObjectStoreTrait>,
    http: reqwest::Client,
}

impl FakeRunner {
    pub fn new(
        dispatcher_url: String,
        query_service_url: String,
        bucket: String,
        object_store: std::sync::Arc<dyn ObjectStoreTrait>,
    ) -> Self {
        Self {
            dispatcher: DispatcherClient::new(dispatcher_url),
            query_service_url,
            bucket,
            object_store,
            http: reqwest::Client::new(),
        }
    }

    pub async fn run(&self, invocation: &UdfInvocationPayload) -> anyhow::Result<()> {
        if let Some(spec_value) = invocation.work_payload.get("alert_evaluate") {
            let spec: AlertEvaluateSpec =
                serde_json::from_value(spec_value.clone()).context("decode alert_evaluate spec")?;
            return self.run_alert_evaluate(invocation, spec).await;
        }

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

        self.complete(invocation, "success")
            .await
            .context("complete")?;
        Ok(())
    }

    async fn run_alert_evaluate(
        &self,
        invocation: &UdfInvocationPayload,
        spec: AlertEvaluateSpec,
    ) -> anyhow::Result<()> {
        let rows = self
            .task_query(invocation, &spec)
            .await
            .context("task query")?;

        let outcome = if spec.retry_once && invocation.attempt == 1 {
            "retryable_error"
        } else {
            "success"
        };

        let key = format!(
            "batches/{}/{}/alert_evaluate.jsonl",
            invocation.task_id, invocation.attempt
        );
        let batch_uri = format!("s3://{}/{}", self.bucket, key);

        let bytes = if spec.emit_malformed_output {
            let bad_line = serde_json::json!({
                "alert_definition_id": spec.alert_definition_id,
                "dedupe_key": format!("alert_eval:{}:{}:malformed", spec.alert_definition_id, spec.dataset_version),
                "event_time": Utc::now(),
                "chain_id": 1,
                "block_number": 1,
                "block_hash": "0xdeadbeef",
                "tx_hash": "0xdeadbeef",
                "payload": "not-an-object",
            });
            let mut bytes = serde_json::to_vec(&bad_line).context("encode malformed row")?;
            bytes.push(b'\n');
            bytes
        } else {
            build_alert_events_jsonl(&spec, rows)?
        };

        if let Err(err) = validate_alert_events_jsonl(&bytes) {
            tracing::warn!(
                event = "harness.runner.output.invalid",
                error = %err,
                task_id = %invocation.task_id,
                attempt = invocation.attempt,
                "invalid UDF output; failing task"
            );
            self.complete(invocation, "fatal_error").await?;
            return Ok(());
        }

        self.object_store
            .put_bytes(&self.bucket, &key, bytes.clone(), CONTENT_TYPE_JSONL)
            .await
            .context("upload batch")?;

        self.buffer_publish(invocation, &batch_uri, bytes.len())
            .await
            .context("buffer publish")?;

        self.complete(invocation, outcome)
            .await
            .context("complete")?;
        Ok(())
    }

    async fn task_query(
        &self,
        invocation: &UdfInvocationPayload,
        spec: &AlertEvaluateSpec,
    ) -> anyhow::Result<Vec<(i64, i64, String)>> {
        let url = format!(
            "{}/v1/task/query",
            self.query_service_url.trim_end_matches('/')
        );
        let req = TaskQueryRequestWire {
            task_id: invocation.task_id,
            attempt: invocation.attempt,
            dataset_id: spec.dataset_id,
            sql: spec.sql.clone(),
            limit: spec.limit,
        };

        let resp = self
            .http
            .post(url)
            .header(TASK_CAPABILITY_HEADER, &invocation.capability_token)
            .json(&req)
            .send()
            .await
            .context("POST /v1/task/query")?
            .error_for_status()
            .context("task query status")?;

        let body = resp
            .json::<TaskQueryResponseWire>()
            .await
            .context("decode task query response")?;

        let mut rows = Vec::new();
        for (idx, row) in body.rows.into_iter().enumerate() {
            let chain_id = row
                .get(0)
                .and_then(|v| v.as_i64())
                .with_context(|| format!("row {} chain_id", idx + 1))?;
            let block_number = row
                .get(1)
                .and_then(|v| v.as_i64())
                .with_context(|| format!("row {} block_number", idx + 1))?;
            let block_hash = row
                .get(2)
                .and_then(|v| v.as_str())
                .with_context(|| format!("row {} block_hash", idx + 1))?
                .to_string();

            rows.push((chain_id, block_number, block_hash));
        }

        Ok(rows)
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

    async fn complete(
        &self,
        invocation: &UdfInvocationPayload,
        outcome: &'static str,
    ) -> anyhow::Result<()> {
        let req = CompleteRequest {
            task_id: invocation.task_id,
            attempt: invocation.attempt,
            lease_token: invocation.lease_token,
            outcome,
            datasets_published: Vec::new(),
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

#[async_trait]
impl RuntimeInvoker for FakeRunner {
    async fn invoke(&self, invocation: &UdfInvocationPayload) -> trace_core::Result<()> {
        self.run(invocation).await.map_err(trace_core::Error::from)
    }
}

fn build_alert_events_jsonl(
    spec: &AlertEvaluateSpec,
    rows: Vec<(i64, i64, String)>,
) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();

    for (chain_id, block_number, block_hash) in rows {
        let row = AlertEventRow {
            alert_definition_id: spec.alert_definition_id,
            dedupe_key: format!(
                "alert_eval:{}:{}:{}",
                spec.alert_definition_id, spec.dataset_version, block_number
            ),
            event_time: Utc::now(),
            chain_id,
            block_number,
            block_hash: block_hash.clone(),
            tx_hash: format!("0xtx{block_number:016x}"),
            payload: serde_json::json!({
                "dataset_uuid": spec.dataset_id,
                "dataset_version": spec.dataset_version,
                "block_hash": block_hash,
            }),
        };

        bytes.extend_from_slice(&serde_json::to_vec(&row).context("encode alert event row")?);
        bytes.push(b'\n');
    }

    Ok(bytes)
}

fn validate_alert_events_jsonl(bytes: &[u8]) -> anyhow::Result<()> {
    let text = std::str::from_utf8(bytes).context("batch must be utf-8")?;
    for (idx, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let row: AlertEventRowWire =
            serde_json::from_str(line).with_context(|| format!("jsonl line {}", idx + 1))?;

        if !row.payload.is_object() {
            return Err(anyhow!("payload must be an object"));
        }
    }
    Ok(())
}
