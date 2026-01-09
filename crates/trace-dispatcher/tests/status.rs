use anyhow::Context;
use chrono::Utc;
use sqlx::postgres::PgPoolOptions;
use trace_dispatcher::chain_sync::apply_chain_sync_yaml;
use trace_dispatcher::status::fetch_chain_sync_status;
use uuid::Uuid;

fn state_database_url() -> String {
    std::env::var("STATE_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://trace:trace@localhost:5433/trace_state".to_string())
}

#[tokio::test]
async fn status_includes_stream_cursor_and_follow_head_window() -> anyhow::Result<()> {
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
    let name = format!("status_test_{}", Uuid::new_v4());

    let yaml = format!(
        r#"
kind: chain_sync
name: {name}
chain_id: {chain_id}
mode:
  kind: follow_head
  from_block: 0
  tail_lag: 5
  head_poll_interval_seconds: 10
  max_head_age_seconds: 3600
streams:
  blocks:
    cryo_dataset_name: blocks
    rpc_pool: standard
    chunk_size: 1000
    max_inflight: 10
"#
    );

    let applied = apply_chain_sync_yaml(&pool, org_id, &yaml).await?;

    sqlx::query(
        r#"
        INSERT INTO state.chain_head_observations (org_id, chain_id, rpc_pool, head_block, observed_at, source)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (org_id, chain_id, rpc_pool) DO UPDATE SET
          head_block = EXCLUDED.head_block,
          observed_at = EXCLUDED.observed_at,
          source = EXCLUDED.source,
          updated_at = now()
        "#,
    )
    .bind(org_id)
    .bind(chain_id)
    .bind("standard")
    .bind(100i64)
    .bind(Utc::now())
    .bind("standard")
    .execute(&pool)
    .await
    .context("insert head observation")?;

    let status = fetch_chain_sync_status(&pool, applied.job_id).await?;
    anyhow::ensure!(status.job_id == applied.job_id, "job_id mismatch");
    anyhow::ensure!(status.org_id == org_id, "org_id mismatch");
    anyhow::ensure!(status.chain_id == chain_id, "chain_id mismatch");
    anyhow::ensure!(status.mode == "follow_head", "mode mismatch");

    anyhow::ensure!(
        status.streams.len() == 1,
        "expected 1 stream, got {}",
        status.streams.len()
    );
    let stream = &status.streams[0];
    anyhow::ensure!(stream.dataset_key == "blocks", "dataset_key mismatch");
    anyhow::ensure!(stream.rpc_pool == "standard", "rpc_pool mismatch");
    anyhow::ensure!(stream.next_block == 0, "expected next_block 0");
    anyhow::ensure!(stream.inflight == 0, "expected inflight 0");
    anyhow::ensure!(stream.completed == 0, "expected completed 0");

    anyhow::ensure!(
        stream.observed_head == Some(100),
        "expected observed_head 100"
    );
    anyhow::ensure!(
        stream.eligible_exclusive == Some(96),
        "expected eligible_exclusive 96"
    );

    Ok(())
}
