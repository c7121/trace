use crate::outbox_id_for_task_wakeup;
use anyhow::Context;
use sqlx::{PgPool, Row};
use uuid::Uuid;

// UUIDv5 namespace for deterministic dataset UUIDs derived from chain_id (Lite ms/13).
const CHAIN_DATASET_NAMESPACE: Uuid = Uuid::from_bytes([
    0x64, 0x6c, 0x64, 0x7a, 0x8f, 0x64, 0x4a, 0x0f, 0x9d, 0x02, 0x7b, 0x6a, 0x5e, 0xb1,
    0x19, 0x1a,
]);

const RANGE_STATUS_SCHEDULED: &str = "scheduled";

/// Planner request for scheduling `cryo_ingest` range tasks.
///
/// `to_block` is exclusive; scheduled ranges are inclusive: `[start, end]`.
#[derive(Debug, Clone)]
pub struct PlanChainSyncRequest {
    pub chain_id: i64,
    pub from_block: i64,
    pub to_block: i64,
    pub chunk_size: i64,
    pub max_inflight: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanChainSyncResult {
    pub scheduled_ranges: i64,
    pub next_block: i64,
}

pub async fn plan_chain_sync(
    pool: &PgPool,
    task_wakeup_queue: &str,
    req: PlanChainSyncRequest,
) -> anyhow::Result<PlanChainSyncResult> {
    validate_request(&req)?;

    let mut tx = pool.begin().await.context("begin planner tx")?;

    // Initialize cursor if needed (idempotent).
    sqlx::query(
        r#"
        INSERT INTO state.chain_sync_cursor (chain_id, next_block)
        VALUES ($1, $2)
        ON CONFLICT (chain_id) DO NOTHING
        "#,
    )
    .bind(req.chain_id)
    .bind(req.from_block)
    .execute(&mut *tx)
    .await
    .context("init chain sync cursor")?;

    // Serialize planner instances per chain via row-level lock.
    let row = sqlx::query(
        r#"
        SELECT next_block
        FROM state.chain_sync_cursor
        WHERE chain_id = $1
        FOR UPDATE
        "#,
    )
    .bind(req.chain_id)
    .fetch_one(&mut *tx)
    .await
    .context("lock cursor row")?;

    let mut next_block: i64 = row.try_get("next_block").context("read next_block")?;
    if next_block < req.from_block {
        next_block = req.from_block;
    }

    let inflight: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM state.chain_sync_scheduled_ranges
        WHERE chain_id = $1
          AND status = $2
        "#,
    )
    .bind(req.chain_id)
    .bind(RANGE_STATUS_SCHEDULED)
    .fetch_one(&mut *tx)
    .await
    .context("count inflight ranges")?;

    let mut remaining = (req.max_inflight - inflight).max(0);
    let mut scheduled = 0_i64;

    while remaining > 0 && next_block < req.to_block {
        let start = next_block;
        let end_exclusive = (start + req.chunk_size).min(req.to_block);
        if end_exclusive <= start {
            break;
        }

        let end_inclusive = end_exclusive - 1;
        let task_id = Uuid::new_v4();

        // Record the range first so scheduling is idempotent under restart.
        let inserted = sqlx::query(
            r#"
            INSERT INTO state.chain_sync_scheduled_ranges (
              chain_id,
              range_start,
              range_end,
              task_id,
              status
            ) VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (chain_id, range_start, range_end) DO NOTHING
            "#,
        )
        .bind(req.chain_id)
        .bind(start)
        .bind(end_inclusive)
        .bind(task_id)
        .bind(RANGE_STATUS_SCHEDULED)
        .execute(&mut *tx)
        .await
        .context("insert scheduled range")?
        .rows_affected();

        if inserted == 0 {
            // Already scheduled: advance cursor but do not enqueue duplicate work.
            next_block = end_exclusive;
            continue;
        }

        let payload = serde_json::to_value(build_cryo_payload(req.chain_id, start, end_inclusive))
            .context("encode cryo payload")?;

        sqlx::query(
            r#"
            INSERT INTO state.tasks (task_id, status, payload)
            VALUES ($1, 'queued', $2)
            ON CONFLICT (task_id) DO NOTHING
            "#,
        )
        .bind(task_id)
        .bind(payload)
        .execute(&mut *tx)
        .await
        .context("insert cryo task")?;

        let outbox_id = outbox_id_for_task_wakeup(task_id, 1);
        let wakeup = serde_json::json!({ "task_id": task_id });
        sqlx::query(
            r#"
            INSERT INTO state.outbox (outbox_id, topic, payload, available_at)
            VALUES ($1, $2, $3, now())
            ON CONFLICT (outbox_id) DO NOTHING
            "#,
        )
        .bind(outbox_id)
        .bind(task_wakeup_queue)
        .bind(wakeup)
        .execute(&mut *tx)
        .await
        .context("insert wakeup outbox")?;

        next_block = end_exclusive;
        scheduled += 1;
        remaining -= 1;
    }

    sqlx::query(
        r#"
        UPDATE state.chain_sync_cursor
        SET next_block = $2,
            updated_at = now()
        WHERE chain_id = $1
        "#,
    )
    .bind(req.chain_id)
    .bind(next_block)
    .execute(&mut *tx)
    .await
    .context("update cursor")?;

    tx.commit().await.context("commit planner tx")?;
    Ok(PlanChainSyncResult {
        scheduled_ranges: scheduled,
        next_block,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
struct CryoTaskPayload {
    dataset_uuid: Uuid,
    chain_id: i64,
    range_start: i64,
    range_end: i64,
    config_hash: String,
}

fn build_cryo_payload(chain_id: i64, range_start: i64, range_end: i64) -> CryoTaskPayload {
    let dataset_uuid = derive_chain_blocks_dataset_uuid(chain_id);
    CryoTaskPayload {
        dataset_uuid,
        chain_id,
        range_start,
        range_end,
        config_hash: "cryo_ingest.blocks:v1".to_string(),
    }
}

fn derive_chain_blocks_dataset_uuid(chain_id: i64) -> Uuid {
    let name = format!("cryo_ingest.blocks:{chain_id}");
    Uuid::new_v5(&CHAIN_DATASET_NAMESPACE, name.as_bytes())
}

fn validate_request(req: &PlanChainSyncRequest) -> anyhow::Result<()> {
    anyhow::ensure!(req.chain_id > 0, "chain_id must be > 0");
    anyhow::ensure!(req.from_block >= 0, "from_block must be >= 0");
    anyhow::ensure!(req.to_block > req.from_block, "to_block must be > from_block");
    anyhow::ensure!(req.chunk_size > 0, "chunk_size must be > 0");
    anyhow::ensure!(req.max_inflight > 0, "max_inflight must be > 0");
    Ok(())
}
