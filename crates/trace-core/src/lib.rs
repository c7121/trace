//! Shared core abstractions for Trace Lite.
//!
//! This crate defines cross-crate contracts used by the harness and query service: queues, object
//! storage, task capability signing, and query safety gates.
//!
//! # API notes
//! `trace-core` is an internal crate (`publish = false`). Its public API currently uses a few
//! third-party types (`uuid::Uuid`, `chrono::DateTime<Utc>`, `serde_json::Value`) as part of the
//! Trace Lite contract.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, time::Duration};
use uuid::Uuid;

pub mod lite;

#[cfg(feature = "aws")]
pub mod aws;

pub mod query;
pub mod udf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    inner: anyhow::Error,
}

impl Error {
    pub fn msg(message: impl Into<String>) -> Self {
        Self {
            inner: anyhow::anyhow!(message.into()),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

impl From<anyhow::Error> for Error {
    fn from(value: anyhow::Error) -> Self {
        Self { inner: value }
    }
}

impl From<sqlx::Error> for Error {
    fn from(value: sqlx::Error) -> Self {
        Self {
            inner: anyhow::Error::from(value),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueMessage {
    /// Opaque token used to acknowledge or requeue the message.
    ///
    /// - PgQueue: UUID string
    /// - SQS: ReceiptHandle
    pub ack_token: String,

    /// Provider message id (for tracing).
    ///
    /// - PgQueue: UUID string
    /// - SQS: MessageId
    pub message_id: String,

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
    ) -> Result<String>;

    async fn receive(
        &self,
        queue: &str,
        max: i64,
        visibility_timeout: Duration,
    ) -> Result<Vec<QueueMessage>>;

    async fn ack(&self, ack_token: &str) -> Result<()>;

    async fn nack_or_requeue(&self, ack_token: &str, delay: Duration) -> Result<()>;
}

#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put_bytes(
        &self,
        bucket: &str,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<()>;

    async fn get_bytes(&self, bucket: &str, key: &str) -> Result<Vec<u8>>;
}

#[derive(Debug, Clone)]
pub struct TaskCapabilityIssueRequest {
    pub org_id: Uuid,
    pub task_id: Uuid,
    pub attempt: i64,
    pub datasets: Vec<DatasetGrant>,
    pub s3: S3Grants,
}

pub trait Signer: Send + Sync {
    /// Issue a task-scoped capability token (JWT).
    ///
    /// The signer is responsible for setting `iss`, `aud`, `sub`, `iat`, `exp`.
    fn issue_task_capability(&self, req: &TaskCapabilityIssueRequest) -> Result<String>;

    /// Verify and decode a task capability token.
    fn verify_task_capability(&self, token: &str) -> Result<TaskCapabilityClaims>;
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
