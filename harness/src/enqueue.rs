use crate::{config::HarnessConfig, pgqueue::PgQueue};
use anyhow::Context;
use chrono::Utc;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

/// Enqueue a task wakeup message into the harness queue.
///
/// This exists purely to make manual testing ergonomic:
/// - `dispatcher` + `worker` + `sink` can run in separate terminals
/// - you can enqueue tasks without opening psql
pub async fn run(cfg: &HarnessConfig, task_id: Option<Uuid>) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let pgq = PgQueue::new(pool);
    let task_id = task_id.unwrap_or_else(Uuid::new_v4);

    pgq.publish(
        &cfg.task_wakeup_queue,
        serde_json::json!({ "task_id": task_id }),
        Utc::now(),
    )
    .await
    .context("publish task wakeup")?;

    println!("enqueued task_id={task_id}");
    Ok(())
}
