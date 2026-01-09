use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use trace_dispatcher::chain_sync::{apply_chain_sync_yaml, set_chain_sync_enabled};
use uuid::Uuid;

fn state_database_url() -> String {
    std::env::var("STATE_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://trace:trace@localhost:5433/trace_state".to_string())
}

#[tokio::test]
async fn pause_and_resume_toggle_enabled() -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&state_database_url())
        .await
        .context("connect state db")?;

    sqlx::migrate!("../../harness/migrations/state")
        .run(&pool)
        .await
        .context("migrate state db")?;

    let org_id = Uuid::new_v4();
    let chain_id = ((Uuid::new_v4().as_u128() % 1_000_000) as i64) + 1;
    let name = format!("pause_resume_test_{}", Uuid::new_v4());
    let yaml = format!(
        r#"
kind: chain_sync
name: {name}
chain_id: {chain_id}
mode:
  kind: fixed_target
  from_block: 0
  to_block: 1000
streams:
  blocks:
    cryo_dataset_name: blocks
    rpc_pool: standard
    chunk_size: 1000
    max_inflight: 10
"#
    );

    let applied = apply_chain_sync_yaml(&pool, org_id, &yaml).await?;

    let paused_job_id = set_chain_sync_enabled(&pool, org_id, &name, false).await?;
    anyhow::ensure!(
        paused_job_id == applied.job_id,
        "expected pause to return job_id {} got {paused_job_id}",
        applied.job_id
    );

    let enabled: bool = sqlx::query_scalar(
        r#"
        SELECT enabled
        FROM state.chain_sync_jobs
        WHERE job_id = $1
        "#,
    )
    .bind(applied.job_id)
    .fetch_one(&pool)
    .await
    .context("read enabled after pause")?;
    anyhow::ensure!(!enabled, "expected enabled=false after pause");

    let resumed_job_id = set_chain_sync_enabled(&pool, org_id, &name, true).await?;
    anyhow::ensure!(
        resumed_job_id == applied.job_id,
        "expected resume to return job_id {} got {resumed_job_id}",
        applied.job_id
    );

    let enabled: bool = sqlx::query_scalar(
        r#"
        SELECT enabled
        FROM state.chain_sync_jobs
        WHERE job_id = $1
        "#,
    )
    .bind(applied.job_id)
    .fetch_one(&pool)
    .await
    .context("read enabled after resume")?;
    anyhow::ensure!(enabled, "expected enabled=true after resume");

    Ok(())
}

#[tokio::test]
async fn pause_unknown_job_fails() -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&state_database_url())
        .await
        .context("connect state db")?;

    sqlx::migrate!("../../harness/migrations/state")
        .run(&pool)
        .await
        .context("migrate state db")?;

    let err = set_chain_sync_enabled(&pool, Uuid::new_v4(), "missing_job", false)
        .await
        .expect_err("expected missing job to error");
    anyhow::ensure!(
        err.to_string().contains("chain_sync job not found"),
        "unexpected error: {err}"
    );

    Ok(())
}
