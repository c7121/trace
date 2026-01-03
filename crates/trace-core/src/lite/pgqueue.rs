use crate::{Queue, QueueMessage};
use anyhow::Context;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgQueue {
    pool: PgPool,
}

pub type Message = QueueMessage;

impl PgQueue {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn publish(
        &self,
        queue: &str,
        payload: Value,
        available_at: DateTime<Utc>,
    ) -> anyhow::Result<Uuid> {
        let message_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO state.queue_messages (message_id, queue_name, payload, available_at)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(message_id)
        .bind(queue)
        .bind(payload)
        .bind(available_at)
        .execute(&self.pool)
        .await
        .with_context(|| format!("pgqueue publish to queue={queue}"))?;

        Ok(message_id)
    }

    pub async fn receive(
        &self,
        queue: &str,
        max: i64,
        visibility_timeout: Duration,
    ) -> anyhow::Result<Vec<Message>> {
        let visibility_millis = duration_millis(visibility_timeout);
        let rows = sqlx::query(
            r#"
            WITH picked AS (
              SELECT message_id
              FROM state.queue_messages
              WHERE queue_name = $1
                AND available_at <= now()
                AND (invisible_until IS NULL OR invisible_until <= now())
              ORDER BY available_at, created_at
              LIMIT $2
              FOR UPDATE SKIP LOCKED
            )
            UPDATE state.queue_messages AS m
            SET invisible_until = now() + ($3::text || ' milliseconds')::interval,
                deliveries = deliveries + 1
            FROM picked
            WHERE m.message_id = picked.message_id
            RETURNING m.message_id, m.queue_name, m.payload, m.deliveries
            "#,
        )
        .bind(queue)
        .bind(max)
        .bind(visibility_millis)
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("pgqueue receive from queue={queue}"))?;

        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            messages.push(Message {
                message_id: row.try_get("message_id")?,
                queue_name: row.try_get("queue_name")?,
                payload: row.try_get("payload")?,
                deliveries: row.try_get("deliveries")?,
            });
        }

        Ok(messages)
    }

    pub async fn ack(&self, message_id: Uuid) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            DELETE FROM state.queue_messages
            WHERE message_id = $1
            "#,
        )
        .bind(message_id)
        .execute(&self.pool)
        .await
        .context("pgqueue ack")?;

        Ok(())
    }

    pub async fn nack_or_requeue(&self, message_id: Uuid, delay: Duration) -> anyhow::Result<()> {
        let delay_millis = duration_millis(delay);
        sqlx::query(
            r#"
            UPDATE state.queue_messages
            SET available_at = now() + ($2::text || ' milliseconds')::interval,
                invisible_until = NULL
            WHERE message_id = $1
            "#,
        )
        .bind(message_id)
        .bind(delay_millis)
        .execute(&self.pool)
        .await
        .context("pgqueue nack_or_requeue")?;

        Ok(())
    }
}

#[async_trait]
impl Queue for PgQueue {
    async fn publish(
        &self,
        queue: &str,
        payload: Value,
        available_at: DateTime<Utc>,
    ) -> anyhow::Result<Uuid> {
        self.publish(queue, payload, available_at).await
    }

    async fn receive(
        &self,
        queue: &str,
        max: i64,
        visibility_timeout: Duration,
    ) -> anyhow::Result<Vec<QueueMessage>> {
        self.receive(queue, max, visibility_timeout).await
    }

    async fn ack(&self, message_id: Uuid) -> anyhow::Result<()> {
        self.ack(message_id).await
    }

    async fn nack_or_requeue(&self, message_id: Uuid, delay: Duration) -> anyhow::Result<()> {
        self.nack_or_requeue(message_id, delay).await
    }
}

fn duration_millis(d: Duration) -> i64 {
    let ms = d.as_millis();
    if ms > i64::MAX as u128 {
        i64::MAX
    } else {
        ms as i64
    }
}

