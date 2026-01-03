use crate::{config::HarnessConfig, pgqueue::PgQueue, s3::ObjectStore};
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct TaskWakeup {
    task_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct ClaimResponse {
    task_id: Uuid,
    attempt: i64,
    lease_token: Uuid,
    capability_token: String,
}

#[derive(Debug, Serialize)]
struct BufferPublishRequest {
    task_id: Uuid,
    attempt: i64,
    lease_token: Uuid,
    batch_uri: String,
    content_type: String,
    batch_size_bytes: i64,
    dedupe_scope: String,
}

#[derive(Debug, Serialize)]
struct CompleteRequest {
    task_id: Uuid,
    attempt: i64,
    lease_token: Uuid,
    outcome: &'static str,
}

#[derive(Debug, Serialize)]
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

pub async fn run(cfg: &HarnessConfig) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;
    let pgq = PgQueue::new(pool);
    let object_store = ObjectStore::new(&cfg.s3_endpoint).context("init object store")?;
    let http = reqwest::Client::new();

    let poll_interval = Duration::from_millis(cfg.worker_poll_ms);
    let visibility_timeout = Duration::from_secs(cfg.worker_visibility_timeout_secs);
    let requeue_delay = Duration::from_millis(cfg.worker_requeue_delay_ms);

    tracing::info!(
        queue = %cfg.task_wakeup_queue,
        dispatcher = %cfg.dispatcher_url,
        "worker started"
    );

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("worker shutting down");
                return Ok(());
            }
            res = pgq.receive(&cfg.task_wakeup_queue, 1, visibility_timeout) => {
                let messages = res?;
                if messages.is_empty() {
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }

                for msg in messages {
                    if let Err(err) = handle_message(cfg, &pgq, &object_store, &http, msg, requeue_delay).await {
                        tracing::warn!(error = %err, "worker message handling failed");
                    }
                }
            }
        }
    }
}

async fn handle_message(
    cfg: &HarnessConfig,
    pgq: &PgQueue,
    object_store: &ObjectStore,
    http: &reqwest::Client,
    msg: crate::pgqueue::Message,
    requeue_delay: Duration,
) -> anyhow::Result<()> {
    let message_id = msg.message_id;
    let wakeup: TaskWakeup = match serde_json::from_value(msg.payload.clone()) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(error = %err, message_id = %message_id, "invalid wakeup payload; dropping");
            pgq.ack(message_id).await?;
            return Ok(());
        }
    };

    let res: anyhow::Result<()> = async {
        let claim_url = format!(
            "{}/internal/task-claim",
            cfg.dispatcher_url.trim_end_matches('/')
        );
        let resp = http
            .post(claim_url)
            .json(&serde_json::json!({ "task_id": wakeup.task_id }))
            .send()
            .await
            .context("POST /internal/task-claim")?;

        if resp.status() == reqwest::StatusCode::CONFLICT {
            pgq.ack(message_id).await?;
            return Ok(());
        }

        let resp = resp.error_for_status().context("task-claim status")?;
        let claim: ClaimResponse = resp.json().await.context("decode task-claim")?;

        let (batch_uri, batch_bytes) = build_batch(cfg, &claim)?;
        let (bucket, key) = crate::s3::parse_s3_uri(&batch_uri).context("parse batch uri")?;
        object_store
            .put_bytes(&bucket, &key, batch_bytes.clone(), "application/jsonl")
            .await
            .context("upload batch")?;

        let publish_url = format!(
            "{}/v1/task/buffer-publish",
            cfg.dispatcher_url.trim_end_matches('/')
        );
        let publish_req = BufferPublishRequest {
            task_id: claim.task_id,
            attempt: claim.attempt,
            lease_token: claim.lease_token,
            batch_uri: batch_uri.clone(),
            content_type: "application/jsonl".to_string(),
            batch_size_bytes: batch_bytes.len().try_into().unwrap_or(i64::MAX),
            dedupe_scope: "harness".to_string(),
        };

        let resp = http
            .post(publish_url)
            .header("X-Trace-Task-Capability", &claim.capability_token)
            .json(&publish_req)
            .send()
            .await
            .context("POST /v1/task/buffer-publish")?;

        if resp.status() == reqwest::StatusCode::CONFLICT {
            pgq.ack(message_id).await?;
            return Ok(());
        }
        resp.error_for_status().context("buffer-publish status")?;

        let complete_url = format!(
            "{}/v1/task/complete",
            cfg.dispatcher_url.trim_end_matches('/')
        );
        let complete_req = CompleteRequest {
            task_id: claim.task_id,
            attempt: claim.attempt,
            lease_token: claim.lease_token,
            outcome: "success",
        };
        let resp = http
            .post(complete_url)
            .header("X-Trace-Task-Capability", &claim.capability_token)
            .json(&complete_req)
            .send()
            .await
            .context("POST /v1/task/complete")?;

        if resp.status() == reqwest::StatusCode::CONFLICT {
            pgq.ack(message_id).await?;
            return Ok(());
        }
        resp.error_for_status().context("complete status")?;

        pgq.ack(message_id).await?;
        Ok(())
    }
    .await;

    match res {
        Ok(()) => Ok(()),
        Err(err) => {
            pgq.nack_or_requeue(message_id, requeue_delay).await?;
            Err(err)
        }
    }
}

fn build_batch(cfg: &HarnessConfig, claim: &ClaimResponse) -> anyhow::Result<(String, Vec<u8>)> {
    let alert_definition_id = Uuid::from_bytes([
        0x49, 0x0b, 0x8f, 0x3f, 0x1d, 0x41, 0x49, 0x6a, 0x91, 0x7b, 0x5b, 0x7e, 0xee, 0xb8, 0x5e,
        0x07,
    ]);
    let dedupe_key = format!("harness:{}", claim.task_id);
    let row = AlertEventRow {
        alert_definition_id,
        dedupe_key,
        event_time: Utc::now(),
        chain_id: 1,
        block_number: 123,
        block_hash: "0xblockhash".to_string(),
        tx_hash: "0xtxhash".to_string(),
        payload: serde_json::json!({
            "task_id": claim.task_id,
            "attempt": claim.attempt,
            "org_id": cfg.org_id,
        }),
    };

    let key = format!("batches/{}/{}.jsonl", claim.task_id, claim.attempt);
    let batch_uri = format!("s3://{}/{}", cfg.s3_bucket, key);

    let mut bytes = serde_json::to_vec(&row).context("encode alert event row")?;
    bytes.push(b'\n');
    Ok((batch_uri, bytes))
}
