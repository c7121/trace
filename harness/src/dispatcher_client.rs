use crate::constants::TASK_CAPABILITY_HEADER;
use anyhow::Context;
use chrono::{DateTime, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use trace_core::DatasetPublication;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct TaskClaimResponse {
    pub task_id: Uuid,
    pub attempt: i64,
    pub lease_token: Uuid,
    pub lease_expires_at: DateTime<Utc>,
    pub capability_token: String,
    pub work_payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct BufferPublishRequest {
    pub task_id: Uuid,
    pub attempt: i64,
    pub lease_token: Uuid,
    pub batch_uri: String,
    pub content_type: String,
    pub batch_size_bytes: i64,
    pub dedupe_scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompleteRequest {
    pub task_id: Uuid,
    pub attempt: i64,
    pub lease_token: Uuid,
    pub outcome: &'static str,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub datasets_published: Vec<DatasetPublication>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteDisposition {
    Ok,
    Conflict,
}

#[derive(Clone, Debug)]
pub struct DispatcherClient {
    base_url: String,
    http: reqwest::Client,
}

impl DispatcherClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            http: reqwest::Client::new(),
        }
    }

    pub async fn task_claim(&self, task_id: Uuid) -> anyhow::Result<Option<TaskClaimResponse>> {
        let url = self.url("/internal/task-claim")?;
        let resp = self
            .http
            .post(url)
            .json(&serde_json::json!({ "task_id": task_id }))
            .send()
            .await
            .context("POST /internal/task-claim")?;

        if resp.status() == reqwest::StatusCode::CONFLICT {
            return Ok(None);
        }

        let resp = resp.error_for_status().context("task-claim status")?;
        Ok(Some(
            resp.json::<TaskClaimResponse>()
                .await
                .context("decode task-claim")?,
        ))
    }

    pub async fn buffer_publish(
        &self,
        capability_token: &str,
        req: &BufferPublishRequest,
    ) -> anyhow::Result<WriteDisposition> {
        let url = self.url("/v1/task/buffer-publish")?;
        let resp = self
            .http
            .post(url)
            .header(TASK_CAPABILITY_HEADER, capability_token)
            .json(req)
            .send()
            .await
            .context("POST /v1/task/buffer-publish")?;

        if resp.status() == reqwest::StatusCode::CONFLICT {
            return Ok(WriteDisposition::Conflict);
        }

        resp.error_for_status().context("buffer-publish status")?;
        Ok(WriteDisposition::Ok)
    }

    pub async fn complete(
        &self,
        capability_token: &str,
        req: &CompleteRequest,
    ) -> anyhow::Result<WriteDisposition> {
        let url = self.url("/v1/task/complete")?;
        let resp = self
            .http
            .post(url)
            .header(TASK_CAPABILITY_HEADER, capability_token)
            .json(req)
            .send()
            .await
            .context("POST /v1/task/complete")?;

        if resp.status() == reqwest::StatusCode::CONFLICT {
            return Ok(WriteDisposition::Conflict);
        }

        resp.error_for_status().context("complete status")?;
        Ok(WriteDisposition::Ok)
    }

    fn url(&self, path: &str) -> anyhow::Result<Url> {
        let base = Url::parse(&self.base_url).context("parse dispatcher base URL")?;
        base.join(path).context("join dispatcher URL")
    }
}
