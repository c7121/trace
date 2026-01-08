use anyhow::Context;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

// UUIDv5 namespace for deterministic task IDs derived from {job_id, dataset_key, range}.
const CHAIN_SYNC_TASK_NAMESPACE: Uuid = Uuid::from_bytes([
    0x90, 0x6c, 0x0c, 0x0c, 0x34, 0x2d, 0x47, 0x2f, 0x97, 0xa1, 0x2c, 0x9b, 0x3d, 0x4e, 0xaa, 0xb1,
]);

const JOB_ERROR_INVALID_CONFIG: &str = "InvalidJobConfig";
const JOB_ERROR_MISSING_HEAD_OBSERVATION: &str = "MissingHeadObservation";
const JOB_ERROR_STALE_HEAD_OBSERVATION: &str = "StaleHeadObservation";

const MAX_JOB_ERROR_MESSAGE_BYTES: usize = 1024;

async fn record_job_error(pool: &PgPool, job_id: Uuid, kind: &str, message: &str) -> anyhow::Result<()> {
    // This is a human-facing operator hint. Keep it short to avoid bloating rows/logs.
    let mut msg = message.to_string();
    if msg.len() > MAX_JOB_ERROR_MESSAGE_BYTES {
        msg.truncate(MAX_JOB_ERROR_MESSAGE_BYTES);
    }

    sqlx::query(
        r#"
        UPDATE state.chain_sync_jobs
        SET last_error_kind = $2,
            last_error_message = $3,
            updated_at = NOW()
        WHERE job_id = $1
          AND (last_error_kind IS DISTINCT FROM $2 OR last_error_message IS DISTINCT FROM $3)
        "#,
    )
    .bind(job_id)
    .bind(kind)
    .bind(msg)
    .execute(pool)
    .await
    .context("update chain_sync_jobs.last_error")?;

    Ok(())
}


#[derive(Debug, Clone, Default)]
pub struct PlannerTickResult {
    pub scheduled_ranges: i64,
}

pub async fn planner_tick_once(
    pool: &PgPool,
    task_wakeup_queue: &str,
) -> anyhow::Result<PlannerTickResult> {
    planner_tick_once_scoped(pool, task_wakeup_queue, None).await
}

pub async fn planner_tick_once_scoped(
    pool: &PgPool,
    task_wakeup_queue: &str,
    org_id_filter: Option<Uuid>,
) -> anyhow::Result<PlannerTickResult> {
    let jobs = sqlx::query(
        r#"
        SELECT job_id, org_id, chain_id, mode, from_block, to_block, tail_lag, max_head_age_seconds
        FROM state.chain_sync_jobs
        WHERE enabled = true
          AND ($1::uuid IS NULL OR org_id = $1)
        ORDER BY updated_at, created_at
        "#,
    )
    .bind(org_id_filter)
    .fetch_all(pool)
    .await
    .context("select enabled chain_sync jobs")?;

    let mut out = PlannerTickResult::default();
    for job in jobs {
        let job_id: Uuid = job.try_get("job_id").context("job_id")?;
        let org_id: Uuid = job.try_get("org_id").context("org_id")?;
        let chain_id: i64 = job.try_get("chain_id").context("chain_id")?;
        let mode: String = job.try_get("mode").context("mode")?;
        let from_block: i64 = job.try_get("from_block").context("from_block")?;
        let to_block: Option<i64> = job.try_get("to_block").context("to_block")?;
        let tail_lag: Option<i64> = job.try_get("tail_lag").context("tail_lag")?;
        let max_head_age_seconds: Option<i32> = job
            .try_get("max_head_age_seconds")
            .context("max_head_age_seconds")?;

        let streams = sqlx::query(
            r#"
            SELECT dataset_key, cryo_dataset_name, rpc_pool, config_hash, chunk_size, max_inflight
            FROM state.chain_sync_streams
            WHERE job_id = $1
            ORDER BY dataset_key
            "#,
        )
        .bind(job_id)
        .fetch_all(pool)
        .await
        .with_context(|| format!("select chain_sync streams job_id={job_id}"))?;

        for stream in streams {
            let dataset_key: String = stream.try_get("dataset_key").context("dataset_key")?;
            let cryo_dataset_name: String = stream
                .try_get("cryo_dataset_name")
                .context("cryo_dataset_name")?;
            let rpc_pool: String = stream.try_get("rpc_pool").context("rpc_pool")?;
            let config_hash: String = stream.try_get("config_hash").context("config_hash")?;
            let chunk_size: Option<i64> = stream.try_get("chunk_size").context("chunk_size")?;
            let max_inflight: Option<i64> =
                stream.try_get("max_inflight").context("max_inflight")?;

            let eligible_exclusive = match mode.as_str() {
                "fixed_target" => {
                    if to_block.is_none() {
                        record_job_error(
                            pool,
                            job_id,
                            JOB_ERROR_INVALID_CONFIG,
                            "fixed_target requires to_block",
                        )
                        .await
                        .ok();
                    }
                    to_block
                }
                "follow_head" => {
                    let Some(max_age) = max_head_age_seconds else {
                        record_job_error(
                            pool,
                            job_id,
                            JOB_ERROR_INVALID_CONFIG,
                            "follow_head requires max_head_age_seconds",
                        )
                        .await
                        .ok();
                        continue;
                    };
                    let Some(tail_lag) = tail_lag else {
                        record_job_error(
                            pool,
                            job_id,
                            JOB_ERROR_INVALID_CONFIG,
                            "follow_head requires tail_lag",
                        )
                        .await
                        .ok();
                        continue;
                    };
                    observed_head_eligible_exclusive(
                        pool,
                        job_id,
                        org_id,
                        chain_id,
                        &rpc_pool,
                        &dataset_key,
                        from_block,
                        tail_lag,
                        max_age as i64,
                    )
                    .await?
                }
                other => {
                    tracing::warn!(
                        event = "trace.dispatcher.chain_sync.mode.unknown",
                        job_id = %job_id,
                        mode = %other,
                        "unknown chain_sync mode; skipping"
                    );
                    None
                }
            };

            let Some(eligible_exclusive) = eligible_exclusive else {
                continue;
            };

            out.scheduled_ranges += plan_stream_once(
                pool,
                task_wakeup_queue,
                job_id,
                org_id,
                chain_id,
                from_block,
                eligible_exclusive,
                &dataset_key,
                &cryo_dataset_name,
                &rpc_pool,
                &config_hash,
                chunk_size.unwrap_or(1_000),
                max_inflight.unwrap_or(10),
            )
            .await?;
        }
    }

    Ok(out)
}

async fn observed_head_eligible_exclusive(
    pool: &PgPool,
    job_id: Uuid,
    org_id: Uuid,
    chain_id: i64,
    rpc_pool: &str,
    dataset_key: &str,
    from_block: i64,
    tail_lag: i64,
    max_head_age_seconds: i64,
) -> anyhow::Result<Option<i64>> {
    let row_opt = sqlx::query(
        r#"
        SELECT head_block, observed_at
        FROM state.chain_head_observations
        WHERE org_id = $1
          AND chain_id = $2
          AND rpc_pool = $3
        "#,
    )
    .bind(org_id)
    .bind(chain_id)
    .bind(rpc_pool)
    .fetch_optional(pool)
    .await
    .context("fetch chain head observation")?;

    let Some(row) = row_opt else {
        let msg = format!(
            "follow_head planning blocked: missing head observation for org_id={org_id} chain_id={chain_id} rpc_pool={rpc_pool} stream={dataset_key}",
        );
        record_job_error(pool, job_id, JOB_ERROR_MISSING_HEAD_OBSERVATION, &msg)
            .await
            .ok();
        return Ok(None);
    };

    let observed_head: i64 = row.try_get("head_block")?;
    let observed_at: DateTime<Utc> = row.try_get("observed_at")?;

    let now = Utc::now();
    let age_seconds = now.signed_duration_since(observed_at).num_seconds().max(0);

    if age_seconds > max_head_age_seconds {
        let msg = format!(
            "follow_head planning blocked: stale head observation for org_id={org_id} chain_id={chain_id} rpc_pool={rpc_pool} stream={dataset_key} observed_at={observed_at:?} age_seconds={age_seconds} max_age_seconds={max_head_age_seconds}",
        );
        record_job_error(pool, job_id, JOB_ERROR_STALE_HEAD_OBSERVATION, &msg)
            .await
            .ok();
        return Ok(None);
    }

    // Eligible window is end-exclusive. We plan ranges in [from_block, eligible_exclusive).
    let eligible_exclusive = std::cmp::max(from_block, (observed_head + 1) - tail_lag);
    Ok(Some(eligible_exclusive))
}

#[allow(clippy::too_many_arguments)]
async fn plan_stream_once(
    pool: &PgPool,
    task_wakeup_queue: &str,
    job_id: Uuid,
    org_id: Uuid,
    chain_id: i64,
    from_block: i64,
    eligible_exclusive: i64,
    dataset_key: &str,
    cryo_dataset_name: &str,
    rpc_pool: &str,
    config_hash: &str,
    chunk_size: i64,
    max_inflight: i64,
) -> anyhow::Result<i64> {
    let dataset_uuid = crate::chain_sync::derive_dataset_uuid(org_id, chain_id, dataset_key)
        .with_context(|| {
            format!(
                "derive dataset uuid job_id={job_id} chain_id={chain_id} dataset_key={dataset_key}"
            )
        })?;

    let mut tx = pool.begin().await.with_context(|| {
        format!("begin plan_stream tx job_id={job_id} dataset_key={dataset_key}")
    })?;

    // Serialize planning per stream via cursor row lock.
    let cursor_row = sqlx::query(
        r#"
        SELECT next_block
        FROM state.chain_sync_cursor
        WHERE job_id = $1
          AND dataset_key = $2
        FOR UPDATE
        "#,
    )
    .bind(job_id)
    .bind(dataset_key)
    .fetch_optional(&mut *tx)
    .await
    .context("lock chain_sync_cursor")?;

    let mut cursor_next: i64 = if let Some(row) = cursor_row {
        row.try_get("next_block").context("next_block")?
    } else {
        sqlx::query(
            r#"
            INSERT INTO state.chain_sync_cursor (job_id, dataset_key, next_block)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(job_id)
        .bind(dataset_key)
        .bind(from_block)
        .execute(&mut *tx)
        .await
        .context("init chain_sync_cursor")?;
        from_block
    };

    if eligible_exclusive <= cursor_next {
        tx.commit().await.context("commit no-op plan_stream")?;
        return Ok(0);
    }

    let inflight: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM state.chain_sync_scheduled_ranges
        WHERE job_id = $1
          AND dataset_key = $2
          AND status <> 'completed'
        "#,
    )
    .bind(job_id)
    .bind(dataset_key)
    .fetch_one(&mut *tx)
    .await
    .context("count inflight scheduled ranges")?;

    let mut slots = max_inflight.saturating_sub(inflight);
    if slots <= 0 {
        tx.commit()
            .await
            .context("commit plan_stream no capacity")?;
        return Ok(0);
    }

    let mut scheduled = 0i64;
    while slots > 0 && cursor_next < eligible_exclusive {
        let range_start = cursor_next;
        let range_end = eligible_exclusive.min(range_start.saturating_add(chunk_size));
        let task_id = chain_sync_task_id(job_id, dataset_key, range_start, range_end);

        let inserted = sqlx::query(
            r#"
            INSERT INTO state.chain_sync_scheduled_ranges (
              job_id, dataset_key, range_start, range_end, task_id, status, updated_at
            ) VALUES ($1, $2, $3, $4, $5, 'scheduled', now())
            ON CONFLICT (job_id, dataset_key, range_start, range_end) DO NOTHING
            "#,
        )
        .bind(job_id)
        .bind(dataset_key)
        .bind(range_start)
        .bind(range_end)
        .bind(task_id)
        .execute(&mut *tx)
        .await
        .context("insert scheduled range")?;

        if inserted.rows_affected() == 1 {
            let payload = serde_json::json!({
                "operator": "cryo_ingest",
                "chain_id": chain_id,
                "dataset_key": dataset_key,
                "dataset_uuid": dataset_uuid,
                "cryo_dataset_name": cryo_dataset_name,
                "rpc_pool": rpc_pool,
                "range_start": range_start,
                "range_end": range_end,
                "config_hash": config_hash,
            });

            ensure_task_row(&mut tx, task_id, payload).await?;

            let outbox_id = crate::outbox_id_for_task_wakeup(task_id, 1);
            let wakeup_payload = serde_json::json!({ "task_id": task_id });
            sqlx::query(
                r#"
                INSERT INTO state.outbox (outbox_id, topic, payload, available_at)
                VALUES ($1, $2, $3, now())
                ON CONFLICT (outbox_id) DO NOTHING
                "#,
            )
            .bind(outbox_id)
            .bind(task_wakeup_queue)
            .bind(wakeup_payload)
            .execute(&mut *tx)
            .await
            .context("insert outbox wakeup")?;

            scheduled += 1;
            slots -= 1;
        }

        cursor_next = range_end;
    }

    sqlx::query(
        r#"
        UPDATE state.chain_sync_cursor
        SET next_block = $3,
            updated_at = now()
        WHERE job_id = $1
          AND dataset_key = $2
        "#,
    )
    .bind(job_id)
    .bind(dataset_key)
    .bind(cursor_next)
    .execute(&mut *tx)
    .await
    .context("update chain_sync_cursor")?;

    tx.commit().await.context("commit plan_stream")?;
    Ok(scheduled)
}

async fn ensure_task_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_id: Uuid,
    payload: Value,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO state.tasks (task_id, status, payload)
        VALUES ($1, 'queued', $2)
        ON CONFLICT (task_id) DO NOTHING
        "#,
    )
    .bind(task_id)
    .bind(payload.clone())
    .execute(&mut **tx)
    .await
    .context("insert task row")?;

    let existing = sqlx::query(
        r#"
        SELECT payload
        FROM state.tasks
        WHERE task_id = $1
        "#,
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await
    .context("fetch task payload")?
    .try_get::<Value, _>("payload")
    .context("read task payload")?;

    if existing != payload {
        anyhow::bail!("task payload mismatch for task_id={task_id}");
    }

    Ok(())
}

fn chain_sync_task_id(job_id: Uuid, dataset_key: &str, range_start: i64, range_end: i64) -> Uuid {
    let name = format!("chain_sync:{job_id}:{dataset_key}:{range_start}:{range_end}");
    Uuid::new_v5(&CHAIN_SYNC_TASK_NAMESPACE, name.as_bytes())
}