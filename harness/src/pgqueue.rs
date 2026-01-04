#[doc(inline)]
pub use trace_core::lite::pgqueue::{Message, PgQueue};

#[cfg(test)]
mod tests {
    use super::PgQueue;
    use crate::config::HarnessConfig;
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;
    use uuid::Uuid;

    #[tokio::test]
    async fn publish_receive_ack_requeue_visibility() -> anyhow::Result<()> {
        let cfg = HarnessConfig::from_env().context("load harness config")?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&cfg.state_database_url)
            .await
            .context("connect state db")?;

        sqlx::migrate!("./migrations/state")
            .run(&pool)
            .await
            .context("migrate state db")?;

        let queue = format!("pgqueue_test_{}", Uuid::new_v4());
        let pgq = PgQueue::new(pool);

        let available_at = chrono::Utc::now() - chrono::Duration::seconds(5);

        let id1 = pgq
            .publish(&queue, serde_json::json!({"n": 1}), available_at)
            .await?;
        let id2 = pgq
            .publish(&queue, serde_json::json!({"n": 2}), available_at)
            .await?;

        let mut got = pgq.receive(&queue, 2, Duration::from_millis(200)).await?;
        got.sort_by_key(|m| m.payload["n"].as_i64().unwrap_or_default());
        anyhow::ensure!(got.len() == 2, "expected 2 messages, got {}", got.len());
        anyhow::ensure!(got[0].message_id == id1 || got[0].message_id == id2);

        pgq.ack(&id1).await?;
        pgq.nack_or_requeue(&id2, Duration::from_millis(200))
            .await?;

        let got2 = pgq.receive(&queue, 10, Duration::from_millis(200)).await?;
        anyhow::ensure!(
            got2.is_empty(),
            "expected no visible messages immediately after requeue"
        );

        tokio::time::sleep(Duration::from_millis(250)).await;

        let got3 = pgq.receive(&queue, 10, Duration::from_millis(200)).await?;
        anyhow::ensure!(got3.len() == 1, "expected 1 message after delay");
        anyhow::ensure!(got3[0].message_id == id2);
        pgq.ack(&id2).await?;

        Ok(())
    }
}
