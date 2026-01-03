use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use uuid::Uuid;

pub mod lite;

#[cfg(feature = "aws")]
pub mod aws;

#[derive(Debug, Clone)]
pub struct QueueMessage {
    pub message_id: Uuid,
    pub queue_name: String,
    pub payload: Value,
    pub deliveries: i32,
}

#[async_trait]
pub trait Queue: Send + Sync {
    async fn publish(
        &self,
        queue: &str,
        payload: Value,
        available_at: DateTime<Utc>,
    ) -> anyhow::Result<Uuid>;

    async fn receive(
        &self,
        queue: &str,
        max: i64,
        visibility_timeout: Duration,
    ) -> anyhow::Result<Vec<QueueMessage>>;

    async fn ack(&self, message_id: Uuid) -> anyhow::Result<()>;

    async fn nack_or_requeue(&self, message_id: Uuid, delay: Duration) -> anyhow::Result<()>;
}

#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put_bytes(
        &self,
        bucket: &str,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> anyhow::Result<()>;

    async fn get_bytes(&self, bucket: &str, key: &str) -> anyhow::Result<Vec<u8>>;
}

pub trait Signer: Send + Sync {
    fn issue_task_capability(&self, task_id: Uuid, attempt: i64) -> anyhow::Result<String>;
    fn verify_task_capability(&self, token: &str) -> anyhow::Result<TaskCapabilityClaims>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCapabilityClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub exp: usize,
    pub iat: usize,

    pub org_id: Uuid,
    pub task_id: Uuid,
    pub attempt: i64,
    pub datasets: Vec<DatasetGrant>,
    pub s3: S3Grants,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetGrant {
    pub dataset_uuid: Uuid,
    pub dataset_version: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Grants {
    pub read_prefixes: Vec<String>,
    pub write_prefixes: Vec<String>,
}

impl S3Grants {
    pub fn empty() -> Self {
        Self {
            read_prefixes: Vec::new(),
            write_prefixes: Vec::new(),
        }
    }
}
