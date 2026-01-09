//! Trace query service (Lite mode).
//!
//! Exposes a constrained `/v1/task/query` endpoint backed by DuckDB, intended for local/harness
//! flows with a fail-closed SQL validator.

use crate::config::QueryServiceConfig;
use crate::duckdb::{DuckDbQueryError, DuckDbSandbox, QueryResultSet};
use anyhow::Context;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use trace_core::lite::jwt::{Hs256TaskCapabilityConfig, TaskCapability};
use trace_core::lite::s3::parse_s3_uri;
use trace_core::lite::s3::ObjectStore as LiteObjectStore;
use trace_core::manifest::DatasetManifestV1;
use trace_core::ObjectStore as ObjectStoreTrait;
use trace_core::Signer as SignerTrait;
use trace_core::{DatasetGrant, DatasetStorageRef, S3Grants};
use uuid::Uuid;

pub mod config;
mod duckdb;

pub const TASK_CAPABILITY_HEADER: &str = "X-Trace-Task-Capability";

// Default and max per-request row limits (defense-in-depth against memory/CPU blowups).
const DEFAULT_LIMIT: usize = 1000;
const MAX_LIMIT: usize = 10_000;

#[derive(Clone)]
pub struct AppState {
    pub cfg: QueryServiceConfig,
    pub signer: TaskCapability,
    pub user_jwt: UserJwtVerifier,
    pub duckdb: DuckDbSandbox,
    pub data_pool: sqlx::PgPool,
    pub object_store: Arc<dyn ObjectStoreTrait>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("cfg", &self.cfg)
            .field("data_pool", &"<PgPool>")
            .field("signer", &"<TaskCapability>")
            .field("user_jwt", &"<UserJwtVerifier>")
            .finish()
    }
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

    let user_jwt = UserJwtVerifier::from_config(&cfg).context("init user jwt verifier")?;

    let duckdb = DuckDbSandbox::new();
    let object_store: Arc<dyn ObjectStoreTrait> =
        Arc::new(LiteObjectStore::new(&cfg.s3_endpoint).context("init object store")?);

    Ok(AppState {
        cfg,
        signer,
        user_jwt,
        duckdb,
        data_pool,
        object_store,
    })
}

pub fn router(state: AppState) -> Router {
    let state = Arc::new(state);
    Router::new()
        .route("/v1/task/query", post(task_query))
        .route("/v1/query", post(user_query))
        .with_state(state)
}

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Debug, Deserialize, Serialize)]
pub struct UserQueryRequest {
    pub dataset_id: Uuid,
    pub sql: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct UserQueryResponse {
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
    let grant = require_dataset_grant(&claims, req.dataset_id)?;

    trace_core::query::validate_sql(&req.sql).map_err(|err| {
        tracing::info!(
            event = "query_service.sql.rejected",
            error = %err,
            "sql rejected"
        );
        ApiError::bad_request("invalid sql")
    })?;

    let storage_ref = authorize_dataset_storage(&state.cfg, &claims.s3, &grant)?;

    let limit = req
        .limit
        .map(|v| v as usize)
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);

    let mut results = match storage_ref {
        DatasetStorageRef::S3 { bucket, prefix, .. } => {
            let parquet_uris = resolve_parquet_uris_from_manifest(
                &state.cfg,
                state.object_store.as_ref(),
                &claims.s3,
                &grant,
                &bucket,
                &prefix,
            )
            .await
            .map_err(|err| {
                tracing::warn!(
                    event = "query_service.manifest.error",
                    kind = ?err.kind,
                    error = %err,
                    "dataset manifest resolution failed"
                );
                match err.kind {
                    ManifestErrorKind::TooLarge => {
                        ApiError::payload_too_large("dataset manifest too large")
                    }
                    ManifestErrorKind::InvalidJson | ManifestErrorKind::InvalidSchema => {
                        ApiError::unprocessable("dataset manifest invalid")
                    }
                    ManifestErrorKind::Unauthorized => {
                        ApiError::forbidden("dataset storage not authorized")
                    }
                    ManifestErrorKind::FetchFailed => {
                        ApiError::internal("dataset manifest fetch failed")
                    }
                }
            })?;

            state
                .duckdb
                .query_with_s3_parquet_uris(&state.cfg, parquet_uris, req.sql, limit + 1)
                .await
        }
        DatasetStorageRef::File { prefix, glob } => {
            let scan = file_scan_target(&prefix, &glob).map_err(|err| {
                tracing::warn!(
                    event = "query_service.file_scan.invalid",
                    error = %err,
                    "invalid file scan target"
                );
                ApiError::unprocessable("dataset storage not authorized")
            })?;
            state
                .duckdb
                .query_with_file_scan(scan, req.sql, limit + 1)
                .await
        }
    }
    .map_err(|err| match err {
        DuckDbQueryError::Attach(err) => {
            tracing::warn!(
                event = "query_service.duckdb.attach_failed",
                error = ?err,
                "duckdb dataset attach failed"
            );
            ApiError::internal("query execution failed")
        }
        DuckDbQueryError::Query(_err) => {
            // Avoid logging raw SQL; DuckDB errors may embed the statement text.
            tracing::warn!(
                event = "query_service.duckdb.query_failed",
                "duckdb query failed"
            );
            ApiError::internal("query execution failed")
        }
    })?;

    let truncated = results.rows.len() > limit;
    if truncated {
        results.rows.truncate(limit);
    }

    insert_query_audit(
        &state.data_pool,
        claims.org_id,
        req.task_id,
        req.dataset_id,
        results.rows.len() as i64,
    )
    .await?;

    Ok(Json(TaskQueryResponse {
        columns: columns_to_response(&results),
        rows: results.rows,
        truncated,
    }))
}

async fn user_query(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<UserQueryRequest>,
) -> Result<Json<UserQueryResponse>, ApiError> {
    let claims = require_user_bearer(&state.user_jwt, &headers)?;
    let grant = require_dataset_grant_user(&claims, req.dataset_id)?;

    trace_core::query::validate_sql(&req.sql).map_err(|err| {
        tracing::info!(
            event = "query_service.sql.rejected",
            error = %err,
            "sql rejected"
        );
        ApiError::bad_request("invalid sql")
    })?;

    let storage_ref = authorize_dataset_storage(&state.cfg, &claims.s3, &grant)?;

    let limit = req
        .limit
        .map(|v| v as usize)
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);

    let mut results = match storage_ref {
        DatasetStorageRef::S3 { bucket, prefix, .. } => {
            let parquet_uris = resolve_parquet_uris_from_manifest(
                &state.cfg,
                state.object_store.as_ref(),
                &claims.s3,
                &grant,
                &bucket,
                &prefix,
            )
            .await
            .map_err(|err| {
                tracing::warn!(
                    event = "query_service.manifest.error",
                    kind = ?err.kind,
                    error = %err,
                    "dataset manifest resolution failed"
                );
                match err.kind {
                    ManifestErrorKind::TooLarge => {
                        ApiError::payload_too_large("dataset manifest too large")
                    }
                    ManifestErrorKind::InvalidJson | ManifestErrorKind::InvalidSchema => {
                        ApiError::unprocessable("dataset manifest invalid")
                    }
                    ManifestErrorKind::Unauthorized => {
                        ApiError::forbidden("dataset storage not authorized")
                    }
                    ManifestErrorKind::FetchFailed => {
                        ApiError::internal("dataset manifest fetch failed")
                    }
                }
            })?;

            state
                .duckdb
                .query_with_s3_parquet_uris(&state.cfg, parquet_uris, req.sql, limit + 1)
                .await
        }
        DatasetStorageRef::File { prefix, glob } => {
            let scan = file_scan_target(&prefix, &glob).map_err(|err| {
                tracing::warn!(
                    event = "query_service.file_scan.invalid",
                    error = %err,
                    "invalid file scan target"
                );
                ApiError::unprocessable("dataset storage not authorized")
            })?;
            state
                .duckdb
                .query_with_file_scan(scan, req.sql, limit + 1)
                .await
        }
    }
    .map_err(|err| match err {
        DuckDbQueryError::Attach(err) => {
            tracing::warn!(
                event = "query_service.duckdb.attach_failed",
                error = ?err,
                "duckdb dataset attach failed"
            );
            ApiError::internal("query execution failed")
        }
        DuckDbQueryError::Query(_err) => {
            tracing::warn!(
                event = "query_service.duckdb.query_failed",
                "duckdb query failed"
            );
            ApiError::internal("query execution failed")
        }
    })?;

    let truncated = results.rows.len() > limit;
    if truncated {
        results.rows.truncate(limit);
    }

    insert_user_query_audit(
        &state.data_pool,
        claims.org_id,
        &claims.sub,
        req.dataset_id,
        results.rows.len() as i64,
    )
    .await?;

    Ok(Json(UserQueryResponse {
        columns: columns_to_response(&results),
        rows: results.rows,
        truncated,
    }))
}

fn file_scan_target(prefix: &str, glob: &str) -> anyhow::Result<String> {
    let prefix = prefix.trim_end_matches('/');
    Ok(format!("{prefix}/{glob}"))
}

#[derive(Debug, Clone, Copy)]
enum ManifestErrorKind {
    TooLarge,
    InvalidJson,
    InvalidSchema,
    Unauthorized,
    FetchFailed,
}

#[derive(Debug)]
struct ManifestError {
    kind: ManifestErrorKind,
    inner: anyhow::Error,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#}", self.inner)
    }
}

async fn resolve_parquet_uris_from_manifest(
    cfg: &QueryServiceConfig,
    object_store: &dyn ObjectStoreTrait,
    s3: &S3Grants,
    grant: &DatasetGrant,
    bucket: &str,
    prefix: &str,
) -> Result<Vec<String>, ManifestError> {
    let manifest_key = manifest_key(prefix);
    let bytes = object_store
        .get_bytes(bucket, &manifest_key)
        .await
        .with_context(|| format!("fetch dataset manifest s3://{bucket}/{manifest_key}"))
        .map_err(|err| ManifestError {
            kind: ManifestErrorKind::FetchFailed,
            inner: err,
        })?;

    if bytes.len() > cfg.max_manifest_bytes {
        return Err(ManifestError {
            kind: ManifestErrorKind::TooLarge,
            inner: anyhow::anyhow!(
                "dataset manifest too large ({} bytes > {})",
                bytes.len(),
                cfg.max_manifest_bytes
            ),
        });
    }

    let manifest: DatasetManifestV1 = serde_json::from_slice(&bytes)
        .context("decode dataset manifest json")
        .map_err(|err| ManifestError {
            kind: ManifestErrorKind::InvalidJson,
            inner: err,
        })?;

    validate_manifest(cfg, grant, prefix, &manifest).map_err(|err| ManifestError {
        kind: ManifestErrorKind::InvalidSchema,
        inner: err,
    })?;

    let mut uris = Vec::with_capacity(manifest.parquet_keys.len());
    for key in manifest.parquet_keys {
        let key = key.trim_start_matches('/').to_string();
        let uri = format!("s3://{bucket}/{key}");
        if !s3_read_allowed(s3, &uri) {
            return Err(ManifestError {
                kind: ManifestErrorKind::Unauthorized,
                inner: anyhow::anyhow!("manifest parquet object not authorized"),
            });
        }
        uris.push(uri);
    }

    Ok(uris)
}

fn manifest_key(prefix: &str) -> String {
    let prefix = prefix.trim_start_matches('/');
    let prefix = prefix.trim_end_matches('/');
    format!("{prefix}/_manifest.json")
}

fn validate_manifest(
    cfg: &QueryServiceConfig,
    grant: &DatasetGrant,
    prefix: &str,
    manifest: &DatasetManifestV1,
) -> anyhow::Result<()> {
    if manifest.version != DatasetManifestV1::VERSION {
        anyhow::bail!("unsupported manifest version {}", manifest.version);
    }
    if manifest.dataset_uuid != grant.dataset_uuid
        || manifest.dataset_version != grant.dataset_version
    {
        anyhow::bail!("manifest does not match dataset grant");
    }

    if manifest.parquet_keys.is_empty() {
        anyhow::bail!("manifest contains no parquet objects");
    }
    if manifest.parquet_keys.len() > cfg.max_manifest_objects {
        anyhow::bail!(
            "manifest too many parquet objects ({} > {})",
            manifest.parquet_keys.len(),
            cfg.max_manifest_objects
        );
    }

    let expected_prefix = prefix.trim_start_matches('/').trim_end_matches('/');
    if expected_prefix.is_empty() {
        anyhow::bail!("dataset prefix must not be empty");
    }
    let expected_prefix = format!("{expected_prefix}/");
    for key in &manifest.parquet_keys {
        if key.contains('\\') {
            anyhow::bail!("manifest parquet key contains backslash");
        }
        let key = key.trim_start_matches('/');
        if !key.starts_with(&expected_prefix) {
            anyhow::bail!("manifest parquet key outside dataset prefix");
        }
        if !key.to_ascii_lowercase().ends_with(".parquet") {
            anyhow::bail!("manifest parquet key missing .parquet suffix");
        }
    }

    Ok(())
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

#[derive(Debug, Clone, Deserialize)]
struct UserJwtClaims {
    iss: String,
    aud: String,
    sub: String,
    exp: usize,
    iat: usize,

    org_id: Uuid,
    datasets: Vec<DatasetGrant>,
    s3: S3Grants,
}

#[derive(Clone)]
pub struct UserJwtVerifier {
    issuer: String,
    audience: String,
    current_kid: String,
    next_kid: Option<String>,
    current_decoding_key: DecodingKey,
    next_decoding_key: Option<DecodingKey>,
}

impl std::fmt::Debug for UserJwtVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let next_decoding_key = self.next_decoding_key.as_ref().map(|_| "<redacted>");
        f.debug_struct("UserJwtVerifier")
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("current_kid", &self.current_kid)
            .field("next_kid", &self.next_kid)
            .field("current_decoding_key", &"<redacted>")
            .field("next_decoding_key", &next_decoding_key)
            .finish()
    }
}

impl UserJwtVerifier {
    pub fn from_config(cfg: &QueryServiceConfig) -> anyhow::Result<Self> {
        if cfg.user_jwt_next_kid.is_some() != cfg.user_jwt_next_secret.is_some() {
            anyhow::bail!("USER_JWT_NEXT_KID and USER_JWT_NEXT_SECRET must be set together");
        }

        Ok(Self {
            issuer: cfg.user_jwt_iss.clone(),
            audience: cfg.user_jwt_aud.clone(),
            current_kid: cfg.user_jwt_kid.clone(),
            next_kid: cfg.user_jwt_next_kid.clone(),
            current_decoding_key: DecodingKey::from_secret(cfg.user_jwt_secret.as_bytes()),
            next_decoding_key: cfg
                .user_jwt_next_secret
                .as_deref()
                .map(|s| DecodingKey::from_secret(s.as_bytes())),
        })
    }

    fn verify(&self, token: &str) -> anyhow::Result<UserJwtClaims> {
        let header = decode_header(token).context("decode jwt header")?;
        let kid = header.kid.as_deref().context("missing jwt kid")?;

        let decoding_key = if kid == self.current_kid {
            &self.current_decoding_key
        } else if self.next_kid.as_deref() == Some(kid) {
            self.next_decoding_key
                .as_ref()
                .context("next jwt key not configured")?
        } else {
            anyhow::bail!("invalid jwt kid");
        };

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(std::slice::from_ref(&self.issuer));
        validation.set_audience(std::slice::from_ref(&self.audience));

        let data =
            decode::<UserJwtClaims>(token, decoding_key, &validation).context("verify jwt")?;
        Ok(data.claims)
    }
}

fn require_user_bearer(
    verifier: &UserJwtVerifier,
    headers: &HeaderMap,
) -> Result<UserJwtClaims, ApiError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("missing authorization"))?;

    let token = value
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::unauthorized("invalid authorization"))?;

    let claims = verifier.verify(token).map_err(|err| {
        tracing::warn!(
            event = "query_service.user_jwt.invalid",
            error = %err,
            "invalid user jwt"
        );
        ApiError::unauthorized("invalid authorization")
    })?;

    // Basic claim sanity checks (defense-in-depth; signature/exp are handled by `jsonwebtoken` validation).
    if claims.exp < claims.iat || claims.iss.is_empty() || claims.aud.is_empty() {
        return Err(ApiError::unauthorized("invalid authorization"));
    }

    Ok(claims)
}

fn require_dataset_grant_user(
    claims: &UserJwtClaims,
    dataset_id: Uuid,
) -> Result<DatasetGrant, ApiError> {
    let mut grants = claims
        .datasets
        .iter()
        .filter(|grant| grant.dataset_uuid == dataset_id)
        .cloned();

    let Some(grant) = grants.next() else {
        return Err(ApiError::forbidden("dataset not authorized"));
    };

    if grants.next().is_some() {
        return Err(ApiError::unprocessable("ambiguous dataset grant"));
    }

    Ok(grant)
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
        tracing::warn!(
            event = "query_service.capability.invalid",
            error = %err,
            "invalid capability token"
        );
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

fn require_dataset_grant(
    claims: &trace_core::TaskCapabilityClaims,
    dataset_id: Uuid,
) -> Result<DatasetGrant, ApiError> {
    claims
        .datasets
        .iter()
        .find(|grant| grant.dataset_uuid == dataset_id)
        .cloned()
        .ok_or_else(|| ApiError::forbidden("dataset not authorized"))
}

fn s3_read_allowed(grants: &S3Grants, uri: &str) -> bool {
    let Ok((uri_bucket, uri_key_raw)) = parse_s3_uri(uri) else {
        return false;
    };
    let uri_key = uri_key_raw.trim_start_matches('/');

    grants.read_prefixes.iter().any(|prefix| {
        let Ok((prefix_bucket, prefix_key_raw)) = parse_s3_uri(prefix) else {
            return false;
        };

        if prefix_bucket != uri_bucket {
            return false;
        }

        // Treat granted prefixes as *directory* prefixes even if the trailing slash was omitted.
        // This prevents prefix collisions (e.g. granting `.../foo` unintentionally grants `.../foo-bar`).
        let mut prefix_key = prefix_key_raw.trim_start_matches('/').to_string();
        if !prefix_key.is_empty() && !prefix_key.ends_with('/') {
            prefix_key.push('/');
        }

        uri_key.starts_with(&prefix_key)
    })
}

fn authorize_dataset_storage(
    cfg: &QueryServiceConfig,
    s3: &S3Grants,
    grant: &DatasetGrant,
) -> Result<DatasetStorageRef, ApiError> {
    let storage_ref = grant
        .storage_ref
        .clone()
        .ok_or_else(|| ApiError::forbidden("dataset storage not authorized"))?;

    let storage_prefix = match &storage_ref {
        DatasetStorageRef::S3 { bucket, prefix, .. } => {
            let prefix_dir = prefix.trim_start_matches('/').trim_end_matches('/');
            if prefix_dir.is_empty() {
                return Err(ApiError::unprocessable("invalid dataset storage prefix"));
            }

            // Confirm the resolved storage location is under the caller's S3 grants.
            format!("s3://{bucket}/{prefix_dir}/")
        }
        DatasetStorageRef::File { prefix, .. } => {
            if !cfg.allow_local_files {
                return Err(ApiError::forbidden("dataset storage not authorized"));
            }

            let Some(root) = cfg.local_file_root.as_deref() else {
                return Err(ApiError::forbidden("dataset storage not authorized"));
            };

            let root = std::path::Path::new(root);
            let root = root
                .canonicalize()
                .map_err(|_| ApiError::forbidden("dataset storage not authorized"))?;

            let prefix_path = std::path::Path::new(prefix);
            let prefix_path = prefix_path
                .canonicalize()
                .map_err(|_| ApiError::forbidden("dataset storage not authorized"))?;

            if !prefix_path.starts_with(&root) {
                return Err(ApiError::forbidden("dataset storage not authorized"));
            }

            format!("file://{prefix}")
        }
    };

    if matches!(storage_ref, DatasetStorageRef::S3 { .. }) && !s3_read_allowed(s3, &storage_prefix)
    {
        return Err(ApiError::forbidden("dataset storage not authorized"));
    }

    Ok(storage_ref)
}

async fn insert_query_audit(
    pool: &sqlx::PgPool,
    org_id: Uuid,
    task_id: Uuid,
    dataset_id: Uuid,
    result_row_count: i64,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO data.query_audit (
          org_id,
          task_id,
          dataset_id,
          columns_accessed,
          result_row_count
        ) VALUES ($1, $2, $3, NULL, $4)
        "#,
    )
    .bind(org_id)
    .bind(task_id)
    .bind(dataset_id)
    .bind(result_row_count)
    .execute(pool)
    .await
    .map_err(|err| {
        tracing::warn!(
            event = "query_service.audit.insert_failed",
            error = %err,
            "audit insert failed"
        );
        ApiError::internal("audit insert failed")
    })?;

    Ok(())
}

async fn insert_user_query_audit(
    pool: &sqlx::PgPool,
    org_id: Uuid,
    user_sub: &str,
    dataset_id: Uuid,
    result_row_count: i64,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO data.user_query_audit (
          org_id,
          user_sub,
          dataset_id,
          columns_accessed,
          result_row_count
        ) VALUES ($1, $2, $3, NULL, $4)
        "#,
    )
    .bind(org_id)
    .bind(user_sub)
    .bind(dataset_id)
    .bind(result_row_count)
    .execute(pool)
    .await
    .map_err(|err| {
        tracing::warn!(
            event = "query_service.user_audit.insert_failed",
            error = %err,
            "user query audit insert failed"
        );
        ApiError::internal("audit insert failed")
    })?;

    Ok(())
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

    fn unprocessable(message: &'static str) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message,
        }
    }

    fn payload_too_large(message: &'static str) -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
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
