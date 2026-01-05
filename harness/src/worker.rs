use crate::{
    config::HarnessConfig,
    dispatcher_client::{
        BufferPublishRequest, CompleteRequest, DispatcherClient, TaskClaimResponse,
        WriteDisposition,
    },
    pgqueue::PgQueue,
    s3::ObjectStore,
};
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use trace_core::{ObjectStore as ObjectStoreTrait, Queue as QueueTrait};
use uuid::Uuid;

use crate::constants::{CONTENT_TYPE_JSONL, DEFAULT_ALERT_DEFINITION_ID};

#[derive(Debug, Deserialize)]
struct TaskWakeup {
    task_id: Uuid,
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

pub async fn run(cfg: &HarnessConfig) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;
    let queue: Arc<dyn QueueTrait> = Arc::new(PgQueue::new(pool));
    let object_store: Arc<dyn ObjectStoreTrait> =
        Arc::new(ObjectStore::new(&cfg.s3_endpoint).context("init object store")?);
    let dispatcher = DispatcherClient::new(cfg.dispatcher_url.clone());

    let poll_interval = Duration::from_millis(cfg.worker_poll_ms);
    let visibility_timeout = Duration::from_secs(cfg.worker_visibility_timeout_secs);
    let requeue_delay = Duration::from_millis(cfg.worker_requeue_delay_ms);

    tracing::info!(
        event = "harness.worker.started",
        queue = %cfg.task_wakeup_queue,
        dispatcher = %cfg.dispatcher_url,
        "worker started"
    );

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!(event = "harness.worker.shutdown", "worker shutting down");
                return Ok(());
            }
            res = queue.receive(&cfg.task_wakeup_queue, 1, visibility_timeout) => {
                let messages = res?;
                if messages.is_empty() {
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }

                for msg in messages {
                    if let Err(err) = handle_message(cfg, queue.as_ref(), object_store.as_ref(), &dispatcher, msg, requeue_delay).await {
                        tracing::warn!(
                            event = "harness.worker.message.error",
                            error = %err,
                            "worker message handling failed"
                        );
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
    dispatcher: &DispatcherClient,
    msg: crate::pgqueue::Message,
    requeue_delay: Duration,
) -> anyhow::Result<()> {
    let message_id = msg.message_id.clone();
    let ack_token = msg.ack_token.clone();
    let wakeup: TaskWakeup = match serde_json::from_value(msg.payload.clone()) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(
                event = "harness.worker.wakeup.invalid",
                error = %err,
                message_id = %message_id,
                "invalid wakeup payload; dropping"
            );
            queue.ack(&ack_token).await?;
            return Ok(());
        }
    };

    let res: anyhow::Result<()> = async {
        let Some(claim) = dispatcher.task_claim(wakeup.task_id).await? else {
            queue.ack(&ack_token).await?;
            return Ok(());
        };

        let (batch_uri, batch_bytes) = build_batch(cfg, &claim)?;
        let (bucket, key) = crate::s3::parse_s3_uri(&batch_uri).context("parse batch uri")?;
        object_store
            .put_bytes(&bucket, &key, batch_bytes.clone(), CONTENT_TYPE_JSONL)
            .await
            .context("upload batch")?;

        let publish_req = BufferPublishRequest {
            task_id: claim.task_id,
            attempt: claim.attempt,
            lease_token: claim.lease_token,
            batch_uri: batch_uri.clone(),
            content_type: CONTENT_TYPE_JSONL.to_string(),
            batch_size_bytes: batch_bytes.len().min(i64::MAX as usize) as i64,
            dedupe_scope: "harness".to_string(),
        };

        if dispatcher
            .buffer_publish(&claim.capability_token, &publish_req)
            .await?
            == WriteDisposition::Conflict
        {
            queue.ack(&ack_token).await?;
            return Ok(());
        }

        let complete_req = CompleteRequest {
            task_id: claim.task_id,
            attempt: claim.attempt,
            lease_token: claim.lease_token,
            outcome: "success",
            datasets_published: Vec::new(),
        };
        if dispatcher
            .complete(&claim.capability_token, &complete_req)
            .await?
            == WriteDisposition::Conflict
        {
            queue.ack(&ack_token).await?;
            return Ok(());
        }

        queue.ack(&ack_token).await?;
        Ok(())
    }
    .await;

    match res {
        Ok(()) => Ok(()),
        Err(err) => {
            queue.nack_or_requeue(&ack_token, requeue_delay).await?;
            Err(err)
        }
    }
}

fn build_batch(
    cfg: &HarnessConfig,
    claim: &TaskClaimResponse,
) -> anyhow::Result<(String, Vec<u8>)> {
    let dedupe_key = format!("harness:{}", claim.task_id);
    let row = AlertEventRow {
        alert_definition_id: DEFAULT_ALERT_DEFINITION_ID,
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
