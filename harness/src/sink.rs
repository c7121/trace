use crate::{config::HarnessConfig, pgqueue::PgQueue, s3::ObjectStore};
use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use trace_core::{ObjectStore as ObjectStoreTrait, Queue as QueueTrait};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct BufferPointerMessage {
    batch_uri: String,
    content_type: Option<String>,
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
    payload: Option<Value>,
}

pub async fn run(cfg: &HarnessConfig) -> anyhow::Result<()> {
    let state_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;
    let data_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.data_database_url)
        .await
        .context("connect data db")?;

    let queue: Arc<dyn QueueTrait> = Arc::new(PgQueue::new(state_pool));
    let object_store: Arc<dyn ObjectStoreTrait> =
        Arc::new(ObjectStore::new(&cfg.s3_endpoint).context("init object store")?);

    let poll_interval = Duration::from_millis(cfg.sink_poll_ms);
    let visibility_timeout = Duration::from_secs(cfg.sink_visibility_timeout_secs);
    let retry_delay = Duration::from_millis(cfg.sink_retry_delay_ms);

    tracing::info!(queue = %cfg.buffer_queue, "sink started");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("sink shutting down");
                return Ok(());
            }
            res = queue.receive(&cfg.buffer_queue, 1, visibility_timeout) => {
                let messages = res?;
                if messages.is_empty() {
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }

                for msg in messages {
                    if let Err(err) = handle_message(cfg, queue.as_ref(), object_store.as_ref(), &data_pool, msg, retry_delay).await {
                        tracing::warn!(error = %err, "sink message handling failed");
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
    data_pool: &PgPool,
    msg: crate::pgqueue::Message,
    retry_delay: Duration,
) -> anyhow::Result<()> {
    let ack_token = msg.ack_token.clone();
    let deliveries = msg.deliveries;

    let res: anyhow::Result<()> = async {
        let pointer: BufferPointerMessage =
            serde_json::from_value(msg.payload.clone()).context("decode buffer pointer payload")?;

        if let Some(ct) = &pointer.content_type {
            if ct != "application/jsonl" {
                return Err(anyhow!("unsupported content_type={ct}"));
            }
        }

        let (bucket, key) = crate::s3::parse_s3_uri(&pointer.batch_uri)?;
        let bytes = object_store
            .get_bytes(&bucket, &key)
            .await
            .context("fetch batch")?;

        let rows = parse_jsonl(&bytes)?;
        insert_alert_events(data_pool, rows).await?;

        queue.ack(&ack_token).await?;
        Ok(())
    }
    .await;

    match res {
        Ok(()) => Ok(()),
        Err(err) => {
            if deliveries >= cfg.sink_max_deliveries {
                let dlq_payload = serde_json::json!({
                    "error": err.to_string(),
                    "original": msg.payload,
                });
                let _ = queue
                    .publish(&cfg.buffer_queue_dlq, dlq_payload, Utc::now())
                    .await?;
                queue.ack(&ack_token).await?;
                return Ok(());
            }

            queue.nack_or_requeue(&ack_token, retry_delay).await?;
            Err(err)
        }
    }
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

        if let Some(payload) = &row.payload {
            if !payload.is_object() {
                return Err(anyhow!("payload must be an object"));
            }
        }

        rows.push(row);
    }

    Ok(rows)
}

async fn insert_alert_events(pool: &PgPool, rows: Vec<AlertEventRow>) -> anyhow::Result<()> {
    let mut tx = pool.begin().await.context("begin data tx")?;

    for row in rows {
        let payload = row.payload.unwrap_or_else(|| serde_json::json!({}));
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
