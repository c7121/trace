use crate::{
    ObjectStore as ObjectStoreTrait, Queue as QueueTrait, QueueMessage, Signer,
    TaskCapabilityClaims, TaskCapabilityIssueRequest,
};
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SqsQueue {
    client: aws_sdk_sqs::Client,
    queue_name: String,
    queue_url: String,
}

impl SqsQueue {
    pub fn new(
        client: aws_sdk_sqs::Client,
        queue_name: impl Into<String>,
        queue_url: impl Into<String>,
    ) -> Self {
        Self {
            client,
            queue_name: queue_name.into(),
            queue_url: queue_url.into(),
        }
    }

    pub async fn from_env(queue_name: impl Into<String>) -> anyhow::Result<Self> {
        let queue_name = queue_name.into();
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = aws_sdk_sqs::Client::new(&config);
        let resp = client
            .get_queue_url()
            .queue_name(&queue_name)
            .send()
            .await
            .context("sqs GetQueueUrl")?;
        let queue_url = resp
            .queue_url()
            .ok_or_else(|| anyhow!("sqs GetQueueUrl returned no queue_url"))?
            .to_string();
        Ok(Self {
            client,
            queue_name,
            queue_url,
        })
    }

    fn ensure_queue(&self, queue: &str) -> anyhow::Result<()> {
        if queue == self.queue_name || queue == self.queue_url {
            return Ok(());
        }
        Err(anyhow!(
            "queue mismatch: got={queue} expected={} or {}",
            self.queue_name,
            self.queue_url
        ))
    }
}

#[async_trait]
impl QueueTrait for SqsQueue {
    async fn publish(
        &self,
        queue: &str,
        payload: Value,
        available_at: DateTime<Utc>,
    ) -> anyhow::Result<String> {
        self.ensure_queue(queue)?;

        let body = serde_json::to_string(&payload).context("serialize sqs payload json")?;
        let delay_secs = {
            let now = Utc::now();
            if available_at <= now {
                0_i32
            } else {
                let delay = available_at - now;
                let delay_secs = delay.num_seconds();
                if delay_secs > 900 {
                    return Err(anyhow!(
                        "sqs delay_seconds max is 900; requested {delay_secs}"
                    ));
                }
                delay_secs.try_into().unwrap_or(900)
            }
        };

        let resp = self
            .client
            .send_message()
            .queue_url(&self.queue_url)
            .message_body(body)
            .delay_seconds(delay_secs)
            .send()
            .await
            .context("sqs SendMessage")?;

        Ok(resp.message_id().unwrap_or_default().to_string())
    }

    async fn receive(
        &self,
        queue: &str,
        max: i64,
        visibility_timeout: Duration,
    ) -> anyhow::Result<Vec<QueueMessage>> {
        self.ensure_queue(queue)?;

        let max_number_of_messages: i32 = max.clamp(1, 10).try_into().unwrap_or(10);
        let visibility_timeout_secs: i32 = visibility_timeout
            .as_secs()
            .min(i32::MAX as u64)
            .try_into()
            .unwrap_or(i32::MAX);

        let resp = self
            .client
            .receive_message()
            .queue_url(&self.queue_url)
            .max_number_of_messages(max_number_of_messages)
            .visibility_timeout(visibility_timeout_secs)
            .message_system_attribute_names(
                aws_sdk_sqs::types::MessageSystemAttributeName::ApproximateReceiveCount,
            )
            .send()
            .await
            .context("sqs ReceiveMessage")?;

        let mut out = Vec::new();
        for message in resp.messages().iter() {
            let ack_token = match message.receipt_handle() {
                Some(v) => v.to_string(),
                None => continue,
            };

            let message_id = message.message_id().unwrap_or_default().to_string();
            let deliveries = message
                .attributes()
                .and_then(|m| {
                    m.get(&aws_sdk_sqs::types::MessageSystemAttributeName::ApproximateReceiveCount)
                })
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(1);

            let body = message.body().unwrap_or_default();
            let payload = serde_json::from_str::<Value>(body)
                .unwrap_or_else(|_| Value::String(body.to_string()));

            out.push(QueueMessage {
                ack_token,
                message_id,
                queue_name: queue.to_string(),
                payload,
                deliveries,
            });
        }

        Ok(out)
    }

    async fn ack(&self, ack_token: &str) -> anyhow::Result<()> {
        self.client
            .delete_message()
            .queue_url(&self.queue_url)
            .receipt_handle(ack_token)
            .send()
            .await
            .context("sqs DeleteMessage")?;
        Ok(())
    }

    async fn nack_or_requeue(&self, ack_token: &str, delay: Duration) -> anyhow::Result<()> {
        let visibility_timeout_secs: i32 = delay
            .as_secs()
            .min(43_200)
            .min(i32::MAX as u64)
            .try_into()
            .unwrap_or(43_200);

        self.client
            .change_message_visibility()
            .queue_url(&self.queue_url)
            .receipt_handle(ack_token)
            .visibility_timeout(visibility_timeout_secs)
            .send()
            .await
            .context("sqs ChangeMessageVisibility")?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct S3ObjectStore {
    client: aws_sdk_s3::Client,
}

impl S3ObjectStore {
    pub fn new(client: aws_sdk_s3::Client) -> Self {
        Self { client }
    }

    pub async fn from_env() -> anyhow::Result<Self> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        Ok(Self {
            client: aws_sdk_s3::Client::new(&config),
        })
    }
}

#[async_trait]
impl ObjectStoreTrait for S3ObjectStore {
    async fn put_bytes(
        &self,
        bucket: &str,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> anyhow::Result<()> {
        self.client
            .put_object()
            .bucket(bucket)
            .key(key)
            .content_type(content_type)
            .body(ByteStream::from(bytes))
            .send()
            .await
            .with_context(|| format!("s3 PutObject bucket={bucket} key={key}"))?;
        Ok(())
    }

    async fn get_bytes(&self, bucket: &str, key: &str) -> anyhow::Result<Vec<u8>> {
        let resp = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .with_context(|| format!("s3 GetObject bucket={bucket} key={key}"))?;

        let bytes = resp
            .body
            .collect()
            .await
            .context("s3 GetObject body collect")?
            .into_bytes()
            .to_vec();
        Ok(bytes)
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
