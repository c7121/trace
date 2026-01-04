use crate::{
    config::HarnessConfig,
    dispatcher_client::{DispatcherClient, TaskClaimResponse},
    pgqueue::PgQueue,
    runner::FakeRunner,
    s3::ObjectStore,
};
use anyhow::Context;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use trace_core::{udf::UdfInvocationPayload, ObjectStore as ObjectStoreTrait, Queue as QueueTrait};
use uuid::Uuid;

use crate::constants::{CONTENT_TYPE_JSON, DEFAULT_ALERT_DEFINITION_ID};

#[derive(Debug, Deserialize)]
struct TaskWakeup {
    task_id: Uuid,
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

    let runner = FakeRunner::new(
        cfg.dispatcher_url.clone(),
        cfg.s3_bucket.clone(),
        object_store.clone(),
    );

    let dispatcher = DispatcherClient::new(cfg.dispatcher_url.clone());

    let poll_interval = Duration::from_millis(cfg.worker_poll_ms);
    let visibility_timeout = Duration::from_secs(cfg.worker_visibility_timeout_secs);
    let requeue_delay = Duration::from_millis(cfg.worker_requeue_delay_ms);

    tracing::info!(
        queue = %cfg.task_wakeup_queue,
        dispatcher = %cfg.dispatcher_url,
        "invoker started"
    );

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("invoker shutting down");
                return Ok(());
            }
            res = queue.receive(&cfg.task_wakeup_queue, 1, visibility_timeout) => {
                let messages = res?;
                if messages.is_empty() {
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }

                for msg in messages {
                    if let Err(err) = handle_message(cfg, queue.as_ref(), object_store.as_ref(), &dispatcher, &runner, msg, requeue_delay).await {
                        tracing::warn!(error = %err, "invoker message handling failed");
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
    runner: &FakeRunner,
    msg: crate::pgqueue::Message,
    requeue_delay: Duration,
) -> anyhow::Result<()> {
    let ack_token = msg.ack_token.clone();
    let message_id = msg.message_id.clone();

    let wakeup: TaskWakeup = match serde_json::from_value(msg.payload.clone()) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(error = %err, message_id = %message_id, "invalid wakeup payload; dropping");
            queue.ack(&ack_token).await?;
            return Ok(());
        }
    };

    let res: anyhow::Result<()> = async {
        let Some(claim) = dispatcher.task_claim(wakeup.task_id).await? else {
            queue.ack(&ack_token).await?;
            return Ok(());
        };

        let bundle_url = ensure_bundle_url(cfg, object_store, &claim).await?;
        let invocation = UdfInvocationPayload {
            task_id: claim.task_id,
            attempt: claim.attempt,
            lease_token: claim.lease_token,
            lease_expires_at: claim.lease_expires_at,
            capability_token: claim.capability_token,
            bundle_url,
            work_payload: claim.work_payload,
        };

        runner.run(&invocation).await?;
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

async fn ensure_bundle_url(
    cfg: &HarnessConfig,
    object_store: &dyn ObjectStoreTrait,
    claim: &TaskClaimResponse,
) -> anyhow::Result<String> {
    if let Some(url) = claim
        .work_payload
        .get("bundle_url")
        .and_then(|v| v.as_str())
    {
        return Ok(url.to_string());
    }
    if let Some(url) = claim
        .work_payload
        .get("bundle_get_url")
        .and_then(|v| v.as_str())
    {
        return Ok(url.to_string());
    }

    let key = if let Some(key) = claim
        .work_payload
        .get("bundle_key")
        .and_then(|v| v.as_str())
    {
        key.to_string()
    } else {
        format!("bundles/{}.json", claim.task_id)
    };

    let bundle = serde_json::json!({
        "alert_definition_id": DEFAULT_ALERT_DEFINITION_ID,
        "dedupe_key": format!("udf:{}", claim.task_id),
        "chain_id": 1,
        "block_number": 123,
        "block_hash": "0xblockhash",
        "tx_hash": "0xtxhash",
        "payload": {
            "task_id": claim.task_id,
            "attempt": claim.attempt,
            "org_id": cfg.org_id,
        },
    });

    let bytes = serde_json::to_vec(&bundle).context("encode default fake bundle")?;

    object_store
        .put_bytes(&cfg.s3_bucket, &key, bytes, CONTENT_TYPE_JSON)
        .await
        .context("upload default fake bundle")?;

    let endpoint = cfg.s3_endpoint.trim_end_matches('/');
    Ok(format!("{endpoint}/{}/{key}", cfg.s3_bucket))
}
