use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ChainSyncStatus {
    pub job_id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub chain_id: i64,
    pub mode: String,
    pub from_block: i64,
    pub to_block: Option<i64>,
    pub streams: Vec<ChainSyncStreamStatus>,
}

#[derive(Debug, Serialize)]
pub struct ChainSyncStreamStatus {
    pub dataset_key: String,
    pub cryo_dataset_name: String,
    pub rpc_pool: String,
    pub next_block: i64,
    pub inflight: i64,
    pub completed: i64,

    pub observed_head: Option<i64>,
    pub eligible_exclusive: Option<i64>,
}

pub async fn fetch_chain_sync_status(
    pool: &PgPool,
    job_id: Uuid,
) -> anyhow::Result<ChainSyncStatus> {
    let job = sqlx::query(
        r#"
        SELECT org_id, name, chain_id, mode, from_block, to_block, tail_lag, max_head_age_seconds
        FROM state.chain_sync_jobs
        WHERE job_id = $1
        "#,
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
    .context("fetch chain_sync job")?;

    let org_id: Uuid = job.try_get("org_id").context("org_id")?;
    let name: String = job.try_get("name").context("name")?;
    let chain_id: i64 = job.try_get("chain_id").context("chain_id")?;
    let mode: String = job.try_get("mode").context("mode")?;
    let from_block: i64 = job.try_get("from_block").context("from_block")?;
    let to_block: Option<i64> = job.try_get("to_block").context("to_block")?;
    let tail_lag: Option<i64> = job.try_get("tail_lag").context("tail_lag")?;
    let max_head_age_seconds: Option<i32> = job
        .try_get("max_head_age_seconds")
        .context("max_head_age_seconds")?;

    let rows = sqlx::query(
        r#"
        WITH inflight AS (
          SELECT job_id, dataset_key, count(*)::bigint AS cnt
          FROM state.chain_sync_scheduled_ranges
          WHERE status <> 'completed'
          GROUP BY job_id, dataset_key
        ),
        completed AS (
          SELECT job_id, dataset_key, count(*)::bigint AS cnt
          FROM state.chain_sync_scheduled_ranges
          WHERE status = 'completed'
          GROUP BY job_id, dataset_key
        )
        SELECT
          s.dataset_key,
          s.cryo_dataset_name,
          s.rpc_pool,
          c.next_block,
          COALESCE(i.cnt, 0) AS inflight,
          COALESCE(d.cnt, 0) AS completed,
          h.head_block AS head_block,
          h.observed_at AS head_observed_at
        FROM state.chain_sync_streams s
        JOIN state.chain_sync_cursor c
          ON c.job_id = s.job_id
         AND c.dataset_key = s.dataset_key
        LEFT JOIN inflight i
          ON i.job_id = s.job_id
         AND i.dataset_key = s.dataset_key
        LEFT JOIN completed d
          ON d.job_id = s.job_id
         AND d.dataset_key = s.dataset_key
        LEFT JOIN state.chain_head_observations h
          ON h.org_id = $2
         AND h.chain_id = $3
         AND h.rpc_pool = s.rpc_pool
        WHERE s.job_id = $1
        ORDER BY s.dataset_key
        "#,
    )
    .bind(job_id)
    .bind(org_id)
    .bind(chain_id)
    .fetch_all(pool)
    .await
    .context("fetch chain_sync stream status")?;

    let now = Utc::now();
    let streams = rows
        .into_iter()
        .map(|row| {
            let dataset_key: String = row.try_get("dataset_key").context("dataset_key")?;
            let cryo_dataset_name: String = row
                .try_get("cryo_dataset_name")
                .context("cryo_dataset_name")?;
            let rpc_pool: String = row.try_get("rpc_pool").context("rpc_pool")?;
            let next_block: i64 = row.try_get("next_block").context("next_block")?;
            let inflight: i64 = row.try_get("inflight").context("inflight")?;
            let completed: i64 = row.try_get("completed").context("completed")?;
            let head_block: Option<i64> = row.try_get("head_block").context("head_block")?;
            let head_observed_at: Option<DateTime<Utc>> = row
                .try_get("head_observed_at")
                .context("head_observed_at")?;

            let (observed_head, eligible_exclusive) = if mode == "follow_head" {
                compute_follow_head_window(
                    head_block,
                    head_observed_at,
                    now,
                    from_block,
                    tail_lag.unwrap_or(0),
                    max_head_age_seconds.unwrap_or(0),
                )
            } else {
                (None, None)
            };

            Ok(ChainSyncStreamStatus {
                dataset_key,
                cryo_dataset_name,
                rpc_pool,
                next_block,
                inflight,
                completed,
                observed_head,
                eligible_exclusive,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(ChainSyncStatus {
        job_id,
        org_id,
        name,
        chain_id,
        mode,
        from_block,
        to_block,
        streams,
    })
}

fn compute_follow_head_window(
    head_block: Option<i64>,
    observed_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    from_block: i64,
    tail_lag: i64,
    max_head_age_seconds: i32,
) -> (Option<i64>, Option<i64>) {
    let Some(head_block) = head_block else {
        return (None, None);
    };
    let Some(observed_at) = observed_at else {
        return (Some(head_block), None);
    };

    let max_age = chrono::Duration::seconds(i64::from(max_head_age_seconds).max(0));
    if observed_at + max_age < now {
        return (Some(head_block), None);
    }

    let eligible = (head_block + 1).saturating_sub(tail_lag.max(0));
    (Some(head_block), Some(from_block.max(eligible)))
}
