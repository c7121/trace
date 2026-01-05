use crate::{
    Error, ObjectStore as ObjectStoreTrait, Queue as QueueTrait, QueueMessage, Result, Signer,
    TaskCapabilityClaims, TaskCapabilityIssueRequest,
};
use anyhow::Context;
use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::time::Duration;

use crate::{runtime::RuntimeInvoker, udf::UdfInvocationPayload};

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

    pub async fn from_env(queue_name: impl Into<String>) -> Result<Self> {
        let queue_name = queue_name.into();
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = aws_sdk_sqs::Client::new(&config);
        let resp = client
            .get_queue_url()
            .queue_name(&queue_name)
            .send()
            .await
            .context("sqs GetQueueUrl")
            .map_err(Error::from)?;
        let queue_url = resp
            .queue_url()
            .ok_or_else(|| Error::msg("sqs GetQueueUrl returned no queue_url"))?
            .to_string();
        Ok(Self {
            client,
            queue_name,
            queue_url,
        })
    }

    fn ensure_queue(&self, queue: &str) -> Result<()> {
        if queue == self.queue_name || queue == self.queue_url {
            return Ok(());
        }
        Err(Error::msg(format!(
            "queue mismatch: got={queue} expected={} or {}",
            self.queue_name, self.queue_url
        )))
    }
}

#[async_trait]
impl QueueTrait for SqsQueue {
    async fn publish(
        &self,
        queue: &str,
        payload: Value,
        available_at: DateTime<Utc>,
    ) -> Result<String> {
        self.ensure_queue(queue)?;

        let body = serde_json::to_string(&payload)
            .context("serialize sqs payload json")
            .map_err(Error::from)?;
        let delay_secs = {
            let now = Utc::now();
            if available_at <= now {
                0_i32
            } else {
                let delay = available_at - now;
                let delay_secs = delay.num_seconds();
                if delay_secs > 900 {
                    return Err(Error::msg(format!(
                        "sqs delay_seconds max is 900; requested {delay_secs}"
                    )));
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
            .context("sqs SendMessage")
            .map_err(Error::from)?;

        Ok(resp.message_id().unwrap_or_default().to_string())
    }

    async fn receive(
        &self,
        queue: &str,
        max: i64,
        visibility_timeout: Duration,
    ) -> Result<Vec<QueueMessage>> {
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
            .context("sqs ReceiveMessage")
            .map_err(Error::from)?;

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

    async fn ack(&self, ack_token: &str) -> Result<()> {
        self.client
            .delete_message()
            .queue_url(&self.queue_url)
            .receipt_handle(ack_token)
            .send()
            .await
            .context("sqs DeleteMessage")
            .map_err(Error::from)?;
        Ok(())
    }

    async fn nack_or_requeue(&self, ack_token: &str, delay: Duration) -> Result<()> {
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
            .context("sqs ChangeMessageVisibility")
            .map_err(Error::from)?;

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

    pub async fn from_env() -> Result<Self> {
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
    ) -> Result<()> {
        self.client
            .put_object()
            .bucket(bucket)
            .key(key)
            .content_type(content_type)
            .body(ByteStream::from(bytes))
            .send()
            .await
            .with_context(|| format!("s3 PutObject bucket={bucket} key={key}"))
            .map_err(Error::from)?;
        Ok(())
    }

    async fn get_bytes(&self, bucket: &str, key: &str) -> Result<Vec<u8>> {
        let resp = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .with_context(|| format!("s3 GetObject bucket={bucket} key={key}"))
            .map_err(Error::from)?;

        let bytes = resp
            .body
            .collect()
            .await
            .context("s3 GetObject body collect")
            .map_err(Error::from)?
            .into_bytes()
            .to_vec();
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Default)]
pub struct KmsSigner;

impl Signer for KmsSigner {
    fn issue_task_capability(&self, _req: &TaskCapabilityIssueRequest) -> Result<String> {
        Err(Error::msg("aws KmsSigner is stubbed (compile-only)"))
    }

    fn verify_task_capability(&self, _token: &str) -> Result<TaskCapabilityClaims> {
        Err(Error::msg("aws KmsSigner is stubbed (compile-only)"))
    }
}

#[derive(Debug, Clone)]
pub struct AwsLambdaInvoker {
    client: aws_sdk_lambda::Client,
    function_name: String,
    qualifier: Option<String>,
}

impl AwsLambdaInvoker {
    pub fn new(
        client: aws_sdk_lambda::Client,
        function_name: impl Into<String>,
        qualifier: Option<String>,
    ) -> Self {
        Self {
            client,
            function_name: function_name.into(),
            qualifier,
        }
    }

    pub async fn from_env(
        function_name: impl Into<String>,
        qualifier: Option<String>,
    ) -> Result<Self> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        Ok(Self {
            client: aws_sdk_lambda::Client::new(&config),
            function_name: function_name.into(),
            qualifier,
        })
    }
}

#[async_trait]
impl RuntimeInvoker for AwsLambdaInvoker {
    async fn invoke(&self, invocation: &UdfInvocationPayload) -> Result<()> {
        let payload = serde_json::to_vec(invocation)
            .context("serialize lambda invocation payload json")
            .map_err(Error::from)?;

        let mut req = self
            .client
            .invoke()
            .function_name(&self.function_name)
            .invocation_type(aws_sdk_lambda::types::InvocationType::RequestResponse)
            .payload(aws_sdk_lambda::primitives::Blob::new(payload));

        if let Some(qualifier) = &self.qualifier {
            req = req.qualifier(qualifier);
        }

        let resp = req
            .send()
            .await
            .context("lambda Invoke")
            .map_err(Error::from)?;

        if let Some(function_error) = resp.function_error() {
            return Err(Error::msg(format!(
                "lambda invocation failed: function_error={function_error}"
            )));
        }

        let status_code = resp.status_code();
        if !(200..300).contains(&status_code) {
            return Err(Error::msg(format!(
                "lambda invocation failed: status_code={status_code}"
            )));
        }

        Ok(())
    }
}
