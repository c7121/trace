//! Trace sink consumer (Lite mode).
//!
//! Consumes buffer pointer messages, fetches JSONL batches from object storage, validates rows,
//! and inserts into the data DB with sink-side idempotency (`dedupe_key` unique).
//!
//! Failure behavior is fail-closed:
//! - Parse/schema errors cause retries up to `max_deliveries`.
//! - Poison batches are sent to DLQ with no partial DB writes.

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use trace_core::{ObjectStore as ObjectStoreTrait, Queue as QueueTrait, QueueMessage};
use uuid::Uuid;

const CONTENT_TYPE_JSONL: &str = "application/jsonl";

#[derive(Clone, Debug)]
pub struct SinkConfig {
    pub buffer_queue: String,
    pub buffer_queue_dlq: String,
    pub poll_interval: Duration,
    pub visibility_timeout: Duration,
    pub retry_delay: Duration,
    pub max_deliveries: i32,
}

#[derive(Debug, Deserialize)]
struct BufferPointerMessage {
    batch_uri: String,
    content_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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

pub struct Sink {
    cfg: SinkConfig,
    queue: Arc<dyn QueueTrait>,
    object_store: Arc<dyn ObjectStoreTrait>,
    data_pool: PgPool,
}

impl Sink {
    pub fn new(
        cfg: SinkConfig,
        queue: Arc<dyn QueueTrait>,
        object_store: Arc<dyn ObjectStoreTrait>,
        data_pool: PgPool,
    ) -> Self {
        Self {
            cfg,
            queue,
            object_store,
            data_pool,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        tracing::info!(
            event = "trace_sink.started",
            queue = %self.cfg.buffer_queue,
            "sink started"
        );

        loop {
            let messages = self
                .queue
                .receive(&self.cfg.buffer_queue, 1, self.cfg.visibility_timeout)
                .await?;
            if messages.is_empty() {
                tokio::time::sleep(self.cfg.poll_interval).await;
                continue;
            }

            for msg in messages {
                if let Err(err) = self.handle_message(msg).await {
                    tracing::warn!(
                        event = "trace_sink.message.error",
                        error = %err,
                        "sink message handling failed"
                    );
                }
            }
        }
    }

    async fn handle_message(&self, msg: QueueMessage) -> anyhow::Result<()> {
        let ack_token = msg.ack_token.clone();
        let deliveries = msg.deliveries;

        let res: anyhow::Result<()> = async {
            let pointer: BufferPointerMessage =
                serde_json::from_value(msg.payload.clone()).context("decode buffer pointer")?;

            if pointer.content_type != CONTENT_TYPE_JSONL {
                return Err(anyhow!(
                    "unsupported content_type={}",
                    pointer.content_type
                ));
            }

            let (bucket, key) = parse_s3_uri(&pointer.batch_uri).context("parse batch_uri")?;
            let bytes = self
                .object_store
                .get_bytes(&bucket, &key)
                .await
                .context("fetch batch")?;

            let rows = parse_jsonl(&bytes)?;
            insert_alert_events(&self.data_pool, rows).await?;

            self.queue.ack(&ack_token).await?;
            Ok(())
        }
        .await;

        match res {
            Ok(()) => Ok(()),
            Err(err) => {
                if deliveries >= self.cfg.max_deliveries {
                    let dlq_payload = serde_json::json!({
                        "error": err.to_string(),
                        "original": msg.payload,
                    });
                    self.queue
                        .publish(&self.cfg.buffer_queue_dlq, dlq_payload, Utc::now())
                        .await?;
                    self.queue.ack(&ack_token).await?;
                    return Ok(());
                }

                self.queue
                    .nack_or_requeue(&ack_token, self.cfg.retry_delay)
                    .await?;
                Err(err)
            }
        }
    }
}

fn parse_s3_uri(uri: &str) -> anyhow::Result<(String, String)> {
    let uri = uri
        .strip_prefix("s3://")
        .ok_or_else(|| anyhow!("batch_uri must start with s3://"))?;
    let (bucket, key) = uri
        .split_once('/')
        .ok_or_else(|| anyhow!("s3 uri missing key"))?;
    Ok((bucket.to_string(), key.to_string()))
}

fn parse_jsonl(bytes: &[u8]) -> anyhow::Result<Vec<AlertEventRow>> {
    let text = std::str::from_utf8(bytes).context("batch must be utf-8")?;
    let mut rows = Vec::new();

    for (idx, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let row: AlertEventRow =
            serde_json::from_str(line).with_context(|| format!("jsonl line {}", idx + 1))?;
        if !row.payload.is_object() {
            return Err(anyhow!("payload must be an object"));
        }

        rows.push(row);
    }

    Ok(rows)
}

async fn insert_alert_events(pool: &PgPool, rows: Vec<AlertEventRow>) -> anyhow::Result<()> {
    let mut tx = pool.begin().await.context("begin data tx")?;

    for row in rows {
        let payload = row.payload;
        sqlx::query(
            r#"
            INSERT INTO data.alert_events (
              dedupe_key,
              alert_definition_id,
              event_time,
              chain_id,
              block_number,
              block_hash,
              tx_hash,
              payload
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            ON CONFLICT (dedupe_key) DO NOTHING
            "#,
        )
        .bind(row.dedupe_key)
        .bind(row.alert_definition_id)
        .bind(row.event_time)
        .bind(row.chain_id)
        .bind(row.block_number)
        .bind(row.block_hash)
        .bind(row.tx_hash)
        .bind(payload)
        .execute(&mut *tx)
        .await
        .context("insert alert event")?;
    }

    tx.commit().await.context("commit data tx")?;
    Ok(())
}
