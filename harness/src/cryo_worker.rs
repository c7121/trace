use crate::config::HarnessConfig;
use crate::dispatcher_client::{CompleteRequest, DispatcherClient, WriteDisposition};
use crate::pgqueue::PgQueue;
use crate::s3::ObjectStore;
use anyhow::Context;
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use trace_core::{DatasetPublication, ObjectStore as ObjectStoreTrait, Queue as QueueTrait};
use uuid::Uuid;

const CONTENT_TYPE_JSON: &str = "application/json";
const CONTENT_TYPE_PARQUET: &str = "application/octet-stream";

// UUIDv5 namespace for deterministic dataset version IDs.
const DATASET_VERSION_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6e, 0x48, 0x4f, 0x2c, 0x56, 0x7a, 0x44, 0xf3, 0x8a, 0x5e, 0xc0, 0x65, 0x0b, 0x2a,
    0x16, 0x9f,
]);

#[derive(Debug, Deserialize)]
struct TaskWakeup {
    task_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryoIngestPayload {
    pub dataset_uuid: Uuid,
    pub chain_id: i64,
    pub range_start: i64,
    pub range_end: i64,
    pub config_hash: String,
}

pub fn derive_dataset_publication(bucket: &str, payload: &CryoIngestPayload) -> DatasetPublication {
    let dataset_version = derive_dataset_version(payload);
    let storage_prefix = format!(
        "s3://{bucket}/cold/datasets/{}/{}/",
        payload.dataset_uuid, dataset_version
    );

    DatasetPublication {
        dataset_uuid: payload.dataset_uuid,
        dataset_version,
        storage_prefix,
        config_hash: payload.config_hash.clone(),
        range_start: payload.range_start,
        range_end: payload.range_end,
    }
}

pub async fn run_task(
    cfg: &HarnessConfig,
    object_store: &dyn ObjectStoreTrait,
    dispatcher: &DispatcherClient,
    task_id: Uuid,
) -> anyhow::Result<Option<DatasetPublication>> {
    let Some(claim) = dispatcher.task_claim(task_id).await? else {
        return Ok(None);
    };

    let payload: CryoIngestPayload =
        serde_json::from_value(claim.work_payload.clone()).context("decode cryo payload")?;

    let pubd = derive_dataset_publication(&cfg.s3_bucket, &payload);

    write_dataset_artifacts(object_store, &pubd, &payload)
        .await
        .context("write dataset artifacts")?;

    let complete_req = CompleteRequest {
        task_id: claim.task_id,
        attempt: claim.attempt,
        lease_token: claim.lease_token,
        outcome: "success",
        datasets_published: vec![pubd.clone()],
    };

    if dispatcher
        .complete(&claim.capability_token, &complete_req)
        .await?
        == WriteDisposition::Conflict
    {
        return Ok(None);
    }

    Ok(Some(pubd))
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
        event = "harness.cryo_worker.started",
        queue = %cfg.task_wakeup_queue,
        dispatcher = %cfg.dispatcher_url,
        "cryo worker started"
    );

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!(event = "harness.cryo_worker.shutdown", "cryo worker shutting down");
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
                            event = "harness.cryo_worker.message.error",
                            error = %err,
                            "cryo worker message handling failed"
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
                event = "harness.cryo_worker.wakeup.invalid",
                error = %err,
                message_id = %message_id,
                "invalid wakeup payload; dropping"
            );
            queue.ack(&ack_token).await?;
            return Ok(());
        }
    };

    let res: anyhow::Result<()> = async {
        let _ = run_task(cfg, object_store, dispatcher, wakeup.task_id).await?;
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

async fn write_dataset_artifacts(
    object_store: &dyn ObjectStoreTrait,
    pubd: &DatasetPublication,
    payload: &CryoIngestPayload,
) -> anyhow::Result<()> {
    let (bucket, prefix_key) =
        crate::s3::parse_s3_uri(&pubd.storage_prefix).context("parse storage prefix")?;

    let file_name = format!("cryo_{}_{}.parquet", payload.range_start, payload.range_end);
    let parquet_key = join_key(&prefix_key, &file_name);
    let parquet_bytes = build_parquet_bytes(payload).await?;
    object_store
        .put_bytes(&bucket, &parquet_key, parquet_bytes, CONTENT_TYPE_PARQUET)
        .await
        .context("upload parquet")?;

    let parquet_uri = format!("s3://{bucket}/{parquet_key}");
    let manifest = serde_json::json!({
        "parquet_objects": [parquet_uri],
    });
    let manifest_key = join_key(&prefix_key, "_manifest.json");
    object_store
        .put_bytes(
            &bucket,
            &manifest_key,
            serde_json::to_vec(&manifest).context("encode manifest")?,
            CONTENT_TYPE_JSON,
        )
        .await
        .context("upload manifest")?;

    Ok(())
}

fn join_key(prefix: &str, leaf: &str) -> String {
    let prefix = prefix.trim_end_matches('/');
    format!("{prefix}/{leaf}")
}

fn derive_dataset_version(payload: &CryoIngestPayload) -> Uuid {
    let name = format!(
        "cryo_ingest:{}:{}:{}:{}:{}",
        payload.dataset_uuid,
        payload.config_hash,
        payload.chain_id,
        payload.range_start,
        payload.range_end
    );
    Uuid::new_v5(&DATASET_VERSION_NAMESPACE, name.as_bytes())
}

async fn build_parquet_bytes(payload: &CryoIngestPayload) -> anyhow::Result<Vec<u8>> {
    let payload = payload.clone();
    tokio::task::spawn_blocking(move || {
        let dir = std::env::temp_dir().join(format!("trace-cryo-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).context("create temp dir")?;
        let parquet_path = dir.join("data.parquet");

        let mut blocks = BTreeSet::new();
        blocks.insert(payload.range_start);
        blocks.insert(payload.range_end);
        blocks.insert(payload.range_start.saturating_add(1).min(payload.range_end));

        let conn = Connection::open_in_memory().context("open duckdb in-memory")?;
        conn.execute_batch(
            r#"
            BEGIN;
            CREATE TABLE blocks (
              chain_id BIGINT NOT NULL,
              block_number BIGINT NOT NULL,
              block_hash VARCHAR NOT NULL
            );
            COMMIT;
            "#,
        )
        .context("create blocks table")?;

        for block_number in blocks {
            let block_hash = format!("0x{block_number:016x}");
            conn.execute(
                "INSERT INTO blocks VALUES (?, ?, ?)",
                duckdb::params![payload.chain_id, block_number, block_hash],
            )
            .context("insert block row")?;
        }

        let parquet_escaped = parquet_path.to_string_lossy().replace('\'', "''");
        conn.execute_batch(&format!(
            "COPY blocks TO '{parquet_escaped}' (FORMAT PARQUET);"
        ))
        .context("copy to parquet")?;

        let bytes = std::fs::read(&parquet_path).context("read parquet bytes")?;
        let _ = std::fs::remove_dir_all(&dir);
        Ok::<_, anyhow::Error>(bytes)
    })
    .await
    .context("join parquet builder")?
}
