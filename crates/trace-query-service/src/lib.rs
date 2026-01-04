use crate::config::QueryServiceConfig;
use crate::duckdb::{default_duckdb_path, DuckDbSandbox, QueryResultSet};
use anyhow::Context;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use trace_core::lite::jwt::{Hs256TaskCapabilityConfig, TaskCapability};
use trace_core::Signer as SignerTrait;
use uuid::Uuid;

pub mod config;
mod duckdb;

pub const TASK_CAPABILITY_HEADER: &str = "X-Trace-Task-Capability";

const DEFAULT_LIMIT: usize = 1000;
const MAX_LIMIT: usize = 10_000;

#[derive(Clone)]
pub struct AppState {
    pub cfg: QueryServiceConfig,
    pub signer: TaskCapability,
    pub duckdb: DuckDbSandbox,
    pub data_pool: sqlx::PgPool,
}

pub async fn build_state(cfg: QueryServiceConfig) -> anyhow::Result<AppState> {
    let data_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.data_database_url)
        .await
        .context("connect data db")?;

    let signer = TaskCapability::from_hs256_config(Hs256TaskCapabilityConfig {
        issuer: cfg.task_capability_iss.clone(),
        audience: cfg.task_capability_aud.clone(),
        current_kid: cfg.task_capability_kid.clone(),
        current_secret: cfg.task_capability_secret.clone(),
        next_kid: cfg.task_capability_next_kid.clone(),
        next_secret: cfg.task_capability_next_secret.clone(),
        ttl: Duration::from_secs(cfg.task_capability_ttl_secs),
    })
    .context("init task capability signer")?;

    let db_path = default_duckdb_path(cfg.duckdb_path.as_deref()).context("choose duckdb path")?;
    let duckdb = DuckDbSandbox::new(db_path, cfg.fixture_rows).context("init duckdb sandbox")?;

    Ok(AppState {
        cfg,
        signer,
        duckdb,
        data_pool,
    })
}

pub fn router(state: AppState) -> Router {
    let state = Arc::new(state);
    Router::new()
        .route("/v1/task/query", post(task_query))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
pub struct TaskQueryRequest {
    pub task_id: Uuid,
    pub attempt: i64,
    pub dataset_id: Uuid,
    pub sql: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct TaskQueryResponse {
    pub columns: Vec<QueryColumnResponse>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct QueryColumnResponse {
    pub name: String,
    pub r#type: String,
}

async fn task_query(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<TaskQueryRequest>,
) -> Result<Json<TaskQueryResponse>, ApiError> {
    let claims = require_task_capability(&state.signer, &headers, req.task_id, req.attempt)?;

    trace_core::query::validate_sql(&req.sql).map_err(|err| {
        tracing::info!(error = %err, "sql rejected");
        ApiError::bad_request("invalid sql")
    })?;

    let limit = req
        .limit
        .map(|v| v as usize)
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);

    let mut results = state
        .duckdb
        .query(req.sql, limit + 1)
        .await
        .map_err(|err| {
            tracing::warn!(error = %err, "duckdb query failed");
            ApiError::internal("query execution failed")
        })?;

    let truncated = results.rows.len() > limit;
    if truncated {
        results.rows.truncate(limit);
    }

    // Audit logging is added in a follow-up commit (Milestone 5).
    let _ = (claims.org_id, req.dataset_id);

    Ok(Json(TaskQueryResponse {
        columns: columns_to_response(&results),
        rows: results.rows,
        truncated,
    }))
}

fn columns_to_response(results: &QueryResultSet) -> Vec<QueryColumnResponse> {
    results
        .columns
        .iter()
        .map(|c| QueryColumnResponse {
            name: c.name.clone(),
            r#type: c.r#type.clone(),
        })
        .collect()
}

fn require_task_capability(
    signer: &dyn SignerTrait,
    headers: &HeaderMap,
    task_id: Uuid,
    attempt: i64,
) -> Result<trace_core::TaskCapabilityClaims, ApiError> {
    let token = headers
        .get(TASK_CAPABILITY_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("missing capability token"))?;

    let claims = signer.verify_task_capability(token).map_err(|err| {
        tracing::warn!(error = %err, "invalid capability token");
        ApiError::unauthorized("invalid capability token")
    })?;

    if claims.task_id != task_id || claims.attempt != attempt {
        return Err(ApiError::forbidden("capability does not match request"));
    }

    let expected_sub = format!("task:{task_id}");
    if claims.sub != expected_sub {
        return Err(ApiError::forbidden("capability does not match request"));
    }

    Ok(claims)
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: &'static str,
}

impl ApiError {
    fn bad_request(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }

    fn unauthorized(message: &'static str) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message,
        }
    }

    fn forbidden(message: &'static str) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message,
        }
    }

    fn internal(message: &'static str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({ "error": self.message }));
        (self.status, body).into_response()
    }
}

