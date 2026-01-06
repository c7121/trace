use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use trace_dispatcher::planner::{plan_chain_sync, PlanChainSyncRequest};
use uuid::Uuid;

fn state_database_url() -> String {
    std::env::var("STATE_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://trace:trace@localhost:5433/trace_state".to_string())
}

fn unique_queue(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4())
}

#[tokio::test]
async fn planner_is_idempotent_under_restarts() -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&state_database_url())
        .await
        .context("connect state db")?;

    sqlx::migrate!("../../harness/migrations/state")
        .run(&pool)
        .await
        .context("migrate state db")?;

    let chain_id = ((Uuid::new_v4().as_u128() % 1_000_000) as i64) + 1;
    let queue = unique_queue("task_wakeup_plan_test");

    let req = PlanChainSyncRequest {
        chain_id,
        from_block: 0,
        to_block: 3_000,
        chunk_size: 1_000,
        max_inflight: 10,
    };

    let r1 = plan_chain_sync(&pool, &queue, req.clone()).await?;
    let r2 = plan_chain_sync(&pool, &queue, req.clone()).await?;

    anyhow::ensure!(r1.scheduled_ranges == 3, "expected 3 scheduled ranges");
    anyhow::ensure!(r2.scheduled_ranges == 0, "expected idempotent second run");

    let scheduled: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM state.chain_sync_scheduled_ranges
        WHERE chain_id = $1
          AND status = 'scheduled'
        "#,
    )
    .bind(chain_id)
    .fetch_one(&pool)
    .await?;

    anyhow::ensure!(scheduled == 3, "expected 3 scheduled ranges, got {scheduled}");

    let next_block: i64 = sqlx::query_scalar(
        r#"
        SELECT next_block
        FROM state.chain_sync_cursor
        WHERE chain_id = $1
        "#,
    )
    .bind(chain_id)
    .fetch_one(&pool)
    .await?;

    anyhow::ensure!(next_block == 3_000, "expected next_block 3000, got {next_block}");

    let outbox: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM state.outbox
        WHERE topic = $1
        "#,
    )
    .bind(&queue)
    .fetch_one(&pool)
    .await?;

    anyhow::ensure!(outbox == 3, "expected 3 outbox rows, got {outbox}");

    Ok(())
}

