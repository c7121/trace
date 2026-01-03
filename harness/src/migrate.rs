use anyhow::Context;
use sqlx::postgres::PgPoolOptions;

use crate::config::HarnessConfig;

/// Run migrations for the harness.
///
/// This is implemented in the skeleton so the next agent can verify connectivity + schema
/// before building the dispatcher/worker logic.
pub async fn run(cfg: &HarnessConfig) -> anyhow::Result<()> {
    let state_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let data_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.data_database_url)
        .await
        .context("connect data db")?;

    sqlx::migrate!("./migrations/state")
        .run(&state_pool)
        .await
        .context("migrate state db")?;

    sqlx::migrate!("./migrations/data")
        .run(&data_pool)
        .await
        .context("migrate data db")?;

    tracing::info!("migrations complete");
    Ok(())
}
