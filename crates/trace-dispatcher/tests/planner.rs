use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use trace_dispatcher::chain_sync::apply_chain_sync_yaml;
use trace_dispatcher::planner::planner_tick_once_scoped;
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

    let queue = unique_queue("task_wakeup_plan_test");

    let org_id = Uuid::new_v4();
    let chain_id = ((Uuid::new_v4().as_u128() % 1_000_000) as i64) + 1;
    let name = format!("planner_test_{}", Uuid::new_v4());
    let yaml = format!(
        r#"
kind: chain_sync
name: {name}
chain_id: {chain_id}
mode:
  kind: fixed_target
  from_block: 0
  to_block: 3000
streams:
  blocks:
    cryo_dataset_name: blocks
    rpc_pool: standard
    chunk_size: 1000
    max_inflight: 10
"#
    );

    let applied = apply_chain_sync_yaml(&pool, org_id, &yaml).await?;

    let r1 = planner_tick_once_scoped(&pool, &queue, Some(org_id)).await?;
    let r2 = planner_tick_once_scoped(&pool, &queue, Some(org_id)).await?;

    anyhow::ensure!(r1.scheduled_ranges == 3, "expected 3 scheduled ranges");
    anyhow::ensure!(r2.scheduled_ranges == 0, "expected idempotent second run");

    let scheduled: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM state.chain_sync_scheduled_ranges
        WHERE job_id = $1
          AND dataset_key = 'blocks'
          AND status <> 'completed'
        "#,
    )
    .bind(applied.job_id)
    .fetch_one(&pool)
    .await?;

    anyhow::ensure!(
        scheduled == 3,
        "expected 3 scheduled ranges, got {scheduled}"
    );

    let next_block: i64 = sqlx::query_scalar(
        r#"
        SELECT next_block
        FROM state.chain_sync_cursor
        WHERE job_id = $1
          AND dataset_key = 'blocks'
        "#,
    )
    .bind(applied.job_id)
    .fetch_one(&pool)
    .await?;

    anyhow::ensure!(
        next_block == 3_000,
        "expected next_block 3000, got {next_block}"
    );

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
