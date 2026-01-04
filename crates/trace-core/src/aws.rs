use crate::{
    ObjectStore as ObjectStoreTrait, Queue as QueueTrait, QueueMessage, Signer,
    TaskCapabilityClaims, TaskCapabilityIssueRequest,
};
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct SqsQueue;

#[async_trait]
impl QueueTrait for SqsQueue {
    async fn publish(
        &self,
        _queue: &str,
        _payload: Value,
        _available_at: DateTime<Utc>,
    ) -> anyhow::Result<String> {
        Err(anyhow!("aws SqsQueue is stubbed (compile-only)"))
    }

    async fn receive(
        &self,
        _queue: &str,
        _max: i64,
        _visibility_timeout: Duration,
    ) -> anyhow::Result<Vec<QueueMessage>> {
        Err(anyhow!("aws SqsQueue is stubbed (compile-only)"))
    }

    async fn ack(&self, _ack_token: &str) -> anyhow::Result<()> {
        Err(anyhow!("aws SqsQueue is stubbed (compile-only)"))
    }

    async fn nack_or_requeue(&self, _ack_token: &str, _delay: Duration) -> anyhow::Result<()> {
        Err(anyhow!("aws SqsQueue is stubbed (compile-only)"))
    }
}

#[derive(Debug, Clone, Default)]
pub struct S3ObjectStore;

#[async_trait]
impl ObjectStoreTrait for S3ObjectStore {
    async fn put_bytes(
        &self,
        _bucket: &str,
        _key: &str,
        _bytes: Vec<u8>,
        _content_type: &str,
    ) -> anyhow::Result<()> {
        Err(anyhow!("aws S3ObjectStore is stubbed (compile-only)"))
    }

    async fn get_bytes(&self, _bucket: &str, _key: &str) -> anyhow::Result<Vec<u8>> {
        Err(anyhow!("aws S3ObjectStore is stubbed (compile-only)"))
    }
}

#[derive(Debug, Clone, Default)]
pub struct KmsSigner;

impl Signer for KmsSigner {
    fn issue_task_capability(&self, _req: &TaskCapabilityIssueRequest) -> anyhow::Result<String> {
        Err(anyhow!("aws KmsSigner is stubbed (compile-only)"))
    }

    fn verify_task_capability(&self, _token: &str) -> anyhow::Result<TaskCapabilityClaims> {
        Err(anyhow!("aws KmsSigner is stubbed (compile-only)"))
    }
}

