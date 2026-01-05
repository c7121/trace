//! Trace dispatcher service (Lite mode).
//!
//! Exposes task-scoped endpoints for claiming work, emitting buffered outputs via an outbox, and
//! tracking lease heartbeats. This crate is intentionally small and designed to be reused by the
//! harness while freezing the dispatcher semantics.

use anyhow::Context;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::watch, task::JoinHandle};
use trace_core::{
    DatasetGrant, DatasetPublication, Queue as QueueTrait, S3Grants, Signer as SignerTrait,
    TaskCapabilityIssueRequest,
};
use uuid::Uuid;

pub const TASK_CAPABILITY_HEADER: &str = "X-Trace-Task-Capability";

// UUIDv5 namespace for deterministic outbox message IDs (task fencing/idempotency).
const OUTBOX_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6c, 0x07, 0x30, 0x87, 0x5b, 0x7c, 0x4c, 0x55, 0xb0, 0x7a, 0x1e, 0x2c, 0x7a, 0x01, 0x5a, 0xe2,
]);

#[derive(Clone, Debug)]
pub struct DispatcherConfig {
    pub org_id: Uuid,
    pub lease_duration_secs: u64,
    pub outbox_poll_ms: u64,
    pub lease_reaper_poll_ms: u64,
    pub outbox_batch_size: i64,
    pub task_wakeup_queue: String,
    pub buffer_queue: String,

    /// Dataset grants to embed in issued task capability tokens (lite/harness default).
    pub default_datasets: Vec<DatasetGrant>,

    /// S3 grants to embed in issued task capability tokens.
    pub default_s3: S3Grants,
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    cfg: DispatcherConfig,
    capability: Arc<dyn SignerTrait>,
    queue: Arc<dyn QueueTrait>,
}

#[derive(Debug)]
pub struct DispatcherServer {
    pub addr: SocketAddr,
    shutdown_tx: watch::Sender<bool>,
    join: JoinHandle<anyhow::Result<()>>,
}

impl DispatcherServer {
    pub async fn start(
        pool: PgPool,
        cfg: DispatcherConfig,
        capability: Arc<dyn SignerTrait>,
        queue: Arc<dyn QueueTrait>,
        bind: SocketAddr,
        enable_outbox: bool,
        enable_lease_reaper: bool,
    ) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(bind)
            .await
            .with_context(|| format!("bind dispatcher to {bind}"))?;
        let addr = listener.local_addr().context("dispatcher local_addr")?;

        let state = Arc::new(AppState {
            pool,
            cfg,
            capability,
            queue,
        });
        let app = build_router(state.clone());

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let join = tokio::spawn(run_dispatcher(
            listener,
            app,
            state,
            shutdown_tx.clone(),
            shutdown_rx,
            enable_outbox,
            enable_lease_reaper,
        ));

        Ok(Self {
            addr,
            shutdown_tx,
            join,
        })
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        let _ = self.shutdown_tx.send(true);
        self.join.await.context("join dispatcher task")??;
        Ok(())
    }
}

async fn run_dispatcher(
    listener: TcpListener,
    app: Router,
    state: Arc<AppState>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    enable_outbox: bool,
    enable_lease_reaper: bool,
) -> anyhow::Result<()> {
    let mut bg = Vec::<JoinHandle<anyhow::Result<()>>>::new();
    if enable_outbox {
        bg.push(tokio::spawn(outbox_drain_loop(
            state.clone(),
            shutdown_rx.clone(),
        )));
    }
    if enable_lease_reaper {
        bg.push(tokio::spawn(lease_reaper_loop(
            state.clone(),
            shutdown_rx.clone(),
        )));
    }

    let mut server_shutdown = shutdown_rx.clone();
    let server =
        axum::serve(listener, app.into_make_service()).with_graceful_shutdown(async move {
            while !*server_shutdown.borrow() {
                if server_shutdown.changed().await.is_err() {
                    break;
                }
            }
        });

    // Ensure the background loops always stop when the server ends (including error paths).
    let server_res = server.await;
    let _ = shutdown_tx.send(true);

    for h in bg {
        let _ = h.await;
    }

    server_res.context("dispatcher serve")?;
    Ok(())
}

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/internal/task-claim", post(task_claim))
        .route("/v1/task/heartbeat", post(task_heartbeat))
        .route("/v1/task/buffer-publish", post(task_buffer_publish))
        .route("/v1/task/complete", post(task_complete))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct TaskClaimRequest {
    task_id: Uuid,
}

#[derive(Debug, Serialize)]
struct TaskClaimResponse {
    task_id: Uuid,
    attempt: i64,
    lease_token: Uuid,
    lease_expires_at: DateTime<Utc>,
    capability_token: String,
    work_payload: Value,
}

async fn task_claim(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TaskClaimRequest>,
) -> ApiResult<Json<TaskClaimResponse>> {
    let now = Utc::now();
    let lease_secs = state.cfg.lease_duration_secs.min(i64::MAX as u64) as i64;
    let lease_expires_at = now + chrono::Duration::seconds(lease_secs);
    let lease_token = Uuid::new_v4();

    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;

    sqlx::query(
        r#"
        INSERT INTO state.tasks (task_id, status, payload)
        VALUES ($1, 'queued', '{}'::jsonb)
        ON CONFLICT (task_id) DO NOTHING
        "#,
    )
    .bind(req.task_id)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    let row = sqlx::query(
        r#"
        SELECT attempt, status, lease_expires_at, payload
        FROM state.tasks
        WHERE task_id = $1
        FOR UPDATE
        "#,
    )
    .bind(req.task_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    let status: String = row.try_get("status").map_err(ApiError::internal)?;
    let current_lease_expires_at: Option<DateTime<Utc>> = row
        .try_get("lease_expires_at")
        .map_err(ApiError::internal)?;

    let (attempt, work_payload) = match status.as_str() {
        "queued" => {
            let row = sqlx::query(
                r#"
                UPDATE state.tasks
                SET status = 'running',
                    lease_token = $2,
                    lease_expires_at = $3,
                    updated_at = now()
                WHERE task_id = $1
                RETURNING attempt, payload
                "#,
            )
            .bind(req.task_id)
            .bind(lease_token)
            .bind(lease_expires_at)
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::internal)?;

            let attempt: i64 = row.try_get("attempt").map_err(ApiError::internal)?;
            let payload: Value = row.try_get("payload").map_err(ApiError::internal)?;
            (attempt, payload)
        }
        "running" => {
            let lease_active = current_lease_expires_at.is_some_and(|t| t > now);
            if lease_active {
                return Err(ApiError::conflict("task already leased"));
            }

            let row = sqlx::query(
                r#"
                UPDATE state.tasks
                SET attempt = attempt + 1,
                    status = 'running',
                    lease_token = $2,
                    lease_expires_at = $3,
                    updated_at = now()
                WHERE task_id = $1
                RETURNING attempt, payload
                "#,
            )
            .bind(req.task_id)
            .bind(lease_token)
            .bind(lease_expires_at)
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::internal)?;

            let attempt: i64 = row.try_get("attempt").map_err(ApiError::internal)?;
            let payload: Value = row.try_get("payload").map_err(ApiError::internal)?;
            (attempt, payload)
        }
        _ => return Err(ApiError::conflict("task not claimable")),
    };

    tx.commit().await.map_err(ApiError::internal)?;

    let capability_req = TaskCapabilityIssueRequest {
        org_id: state.cfg.org_id,
        task_id: req.task_id,
        attempt,
        datasets: state.cfg.default_datasets.clone(),
        s3: state.cfg.default_s3.clone(),
    };

    let capability_token = state
        .capability
        .issue_task_capability(&capability_req)
        .map_err(ApiError::internal)?;

    Ok(Json(TaskClaimResponse {
        task_id: req.task_id,
        attempt,
        lease_token,
        lease_expires_at,
        capability_token,
        work_payload,
    }))
}

#[derive(Debug, Deserialize)]
struct TaskFence {
    task_id: Uuid,
    attempt: i64,
    lease_token: Uuid,
}

#[derive(Debug, Deserialize)]
struct HeartbeatRequest {
    #[serde(flatten)]
    fence: TaskFence,
}

#[derive(Debug, Serialize)]
struct HeartbeatResponse {
    lease_expires_at: DateTime<Utc>,
}

async fn task_heartbeat(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<HeartbeatRequest>,
) -> ApiResult<Json<HeartbeatResponse>> {
    require_task_capability(
        state.capability.as_ref(),
        &headers,
        req.fence.task_id,
        req.fence.attempt,
    )?;

    let now = Utc::now();
    let lease_secs = state.cfg.lease_duration_secs.min(i64::MAX as u64) as i64;
    let lease_expires_at = now + chrono::Duration::seconds(lease_secs);

    let row = sqlx::query(
        r#"
        UPDATE state.tasks
        SET lease_expires_at = $4,
            updated_at = now()
        WHERE task_id = $1
          AND attempt = $2
          AND lease_token = $3
          AND status = 'running'
          AND lease_expires_at > now()
        RETURNING lease_expires_at
        "#,
    )
    .bind(req.fence.task_id)
    .bind(req.fence.attempt)
    .bind(req.fence.lease_token)
    .bind(lease_expires_at)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let Some(row) = row else {
        return Err(ApiError::conflict("stale task fence"));
    };

    Ok(Json(HeartbeatResponse {
        lease_expires_at: row
            .try_get("lease_expires_at")
            .map_err(ApiError::internal)?,
    }))
}

#[derive(Debug, Deserialize)]
struct BufferPublishRequest {
    #[serde(flatten)]
    fence: TaskFence,
    batch_uri: String,
    content_type: String,
    batch_size_bytes: i64,
    dedupe_scope: String,
}

async fn task_buffer_publish(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<BufferPublishRequest>,
) -> ApiResult<StatusCode> {
    require_task_capability(
        state.capability.as_ref(),
        &headers,
        req.fence.task_id,
        req.fence.attempt,
    )?;

    // IMPORTANT: fencing + side-effect writes must be *atomic*.
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;

    let ok = sqlx::query(
        r#"
        SELECT 1
        FROM state.tasks
        WHERE task_id = $1
          AND attempt = $2
          AND lease_token = $3
          AND status = 'running'
          AND lease_expires_at > now()
        FOR UPDATE
        "#,
    )
    .bind(req.fence.task_id)
    .bind(req.fence.attempt)
    .bind(req.fence.lease_token)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::internal)?
    .is_some();

    if !ok {
        return Err(ApiError::conflict("stale task fence"));
    }

    let outbox_id =
        outbox_id_for_buffer_publish(req.fence.task_id, req.fence.attempt, &req.batch_uri);
    let payload = serde_json::json!({
        "task_id": req.fence.task_id,
        "attempt": req.fence.attempt,
        "batch_uri": req.batch_uri,
        "content_type": req.content_type,
        "batch_size_bytes": req.batch_size_bytes,
        "dedupe_scope": req.dedupe_scope,
    });

    sqlx::query(
        r#"
        INSERT INTO state.outbox (outbox_id, topic, payload, available_at)
        VALUES ($1, $2, $3, now())
        ON CONFLICT (outbox_id) DO NOTHING
        "#,
    )
    .bind(outbox_id)
    .bind(&state.cfg.buffer_queue)
    .bind(payload)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    tx.commit().await.map_err(ApiError::internal)?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize)]
struct CompleteRequest {
    #[serde(flatten)]
    fence: TaskFence,
    outcome: TaskOutcome,

    /// Optional dataset publications produced by this task attempt.
    ///
    /// This field is used by Lite ingestion workers to register version-addressed Parquet artifacts
    /// in the state DB on successful completion.
    #[serde(default)]
    datasets_published: Vec<DatasetPublication>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TaskOutcome {
    Success,
    RetryableError,
    FatalError,
}

async fn task_complete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CompleteRequest>,
) -> ApiResult<StatusCode> {
    require_task_capability(
        state.capability.as_ref(),
        &headers,
        req.fence.task_id,
        req.fence.attempt,
    )?;

    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;

    let row = sqlx::query(
        r#"
        SELECT attempt, status, lease_token
        FROM state.tasks
        WHERE task_id = $1
        FOR UPDATE
        "#,
    )
    .bind(req.fence.task_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    let Some(row) = row else {
        return Err(ApiError::conflict("task not found"));
    };

    let current_attempt: i64 = row.try_get("attempt").map_err(ApiError::internal)?;
    let current_status: String = row.try_get("status").map_err(ApiError::internal)?;
    let current_lease_token: Option<Uuid> =
        row.try_get("lease_token").map_err(ApiError::internal)?;

    if current_attempt != req.fence.attempt
        || current_status != "running"
        || current_lease_token != Some(req.fence.lease_token)
    {
        return Err(ApiError::conflict("stale task fence"));
    }

    match req.outcome {
        TaskOutcome::Success => {
            register_dataset_publications(&mut tx, &req.datasets_published).await?;
            sqlx::query(
                r#"
                UPDATE state.tasks
                SET status = 'complete',
                    lease_token = NULL,
                    lease_expires_at = NULL,
                    updated_at = now()
                WHERE task_id = $1
                "#,
            )
            .bind(req.fence.task_id)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        }
        TaskOutcome::FatalError => {
            sqlx::query(
                r#"
                UPDATE state.tasks
                SET status = 'failed',
                    lease_token = NULL,
                    lease_expires_at = NULL,
                    updated_at = now()
                WHERE task_id = $1
                "#,
            )
            .bind(req.fence.task_id)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        }
        TaskOutcome::RetryableError => {
            let row = sqlx::query(
                r#"
                UPDATE state.tasks
                SET attempt = attempt + 1,
                    status = 'queued',
                    lease_token = NULL,
                    lease_expires_at = NULL,
                    updated_at = now()
                WHERE task_id = $1
                RETURNING attempt
                "#,
            )
            .bind(req.fence.task_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::internal)?;

            let new_attempt: i64 = row.try_get("attempt").map_err(ApiError::internal)?;
            let outbox_id = outbox_id_for_task_wakeup(req.fence.task_id, new_attempt);
            let payload = serde_json::json!({ "task_id": req.fence.task_id });
            sqlx::query(
                r#"
                INSERT INTO state.outbox (outbox_id, topic, payload, available_at)
                VALUES ($1, $2, $3, now())
                ON CONFLICT (outbox_id) DO NOTHING
                "#,
            )
            .bind(outbox_id)
            .bind(&state.cfg.task_wakeup_queue)
            .bind(payload)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        }
    }

    tx.commit().await.map_err(ApiError::internal)?;
    Ok(StatusCode::OK)
}

async fn register_dataset_publications(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    datasets: &[DatasetPublication],
) -> ApiResult<()> {
    for pubd in datasets {
        let inserted = sqlx::query(
            r#"
            INSERT INTO state.dataset_versions (
              dataset_version,
              dataset_uuid,
              storage_prefix,
              config_hash,
              range_start,
              range_end
            ) VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (dataset_version) DO NOTHING
            "#,
        )
        .bind(pubd.dataset_version)
        .bind(pubd.dataset_uuid)
        .bind(&pubd.storage_prefix)
        .bind(&pubd.config_hash)
        .bind(pubd.range_start)
        .bind(pubd.range_end)
        .execute(&mut **tx)
        .await
        .map_err(ApiError::internal)?;

        if inserted.rows_affected() == 1 {
            continue;
        }

        // Idempotency under at-least-once: if the dataset version already exists, require that
        // all metadata matches (fail closed on divergence).
        let row = sqlx::query(
            r#"
            SELECT dataset_uuid, storage_prefix, config_hash, range_start, range_end
            FROM state.dataset_versions
            WHERE dataset_version = $1
            "#,
        )
        .bind(pubd.dataset_version)
        .fetch_one(&mut **tx)
        .await
        .map_err(ApiError::internal)?;

        let existing_dataset_uuid: Uuid =
            row.try_get("dataset_uuid").map_err(ApiError::internal)?;
        let existing_storage_prefix: String =
            row.try_get("storage_prefix").map_err(ApiError::internal)?;
        let existing_config_hash: String =
            row.try_get("config_hash").map_err(ApiError::internal)?;
        let existing_range_start: i64 = row.try_get("range_start").map_err(ApiError::internal)?;
        let existing_range_end: i64 = row.try_get("range_end").map_err(ApiError::internal)?;

        let matches = existing_dataset_uuid == pubd.dataset_uuid
            && existing_storage_prefix == pubd.storage_prefix
            && existing_config_hash == pubd.config_hash
            && existing_range_start == pubd.range_start
            && existing_range_end == pubd.range_end;

        if !matches {
            return Err(ApiError::conflict("dataset version conflict"));
        }
    }

    Ok(())
}

fn outbox_id_for_buffer_publish(task_id: Uuid, attempt: i64, batch_uri: &str) -> Uuid {
    let name = format!("buffer_publish:{task_id}:{attempt}:{batch_uri}");
    Uuid::new_v5(&OUTBOX_NAMESPACE, name.as_bytes())
}

fn outbox_id_for_task_wakeup(task_id: Uuid, attempt: i64) -> Uuid {
    let name = format!("task_wakeup:{task_id}:{attempt}");
    Uuid::new_v5(&OUTBOX_NAMESPACE, name.as_bytes())
}

async fn outbox_drain_loop(
    state: Arc<AppState>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let interval = Duration::from_millis(state.cfg.outbox_poll_ms);

    loop {
        if *shutdown_rx.borrow() {
            return Ok(());
        }

        if let Err(err) = drain_outbox_once(
            &state.pool,
            state.queue.as_ref(),
            state.cfg.outbox_batch_size,
        )
        .await
        {
            tracing::warn!(
                event = "harness.dispatcher.outbox_drain.error",
                error = %err,
                "outbox drain error"
            );
        }

        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            _ = shutdown_rx.changed() => {}
        }
    }
}

async fn drain_outbox_once(pool: &PgPool, queue: &dyn QueueTrait, max: i64) -> anyhow::Result<()> {
    let mut tx = pool.begin().await.context("begin outbox drain tx")?;

    let rows = sqlx::query(
        r#"
        SELECT outbox_id, topic, payload, available_at
        FROM state.outbox
        WHERE status = 'pending'
          AND available_at <= now()
        ORDER BY available_at, created_at
        LIMIT $1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(max)
    .fetch_all(&mut *tx)
    .await
    .context("select pending outbox")?;

    for row in rows {
        let outbox_id: Uuid = row.try_get("outbox_id")?;
        let topic: String = row.try_get("topic")?;
        let payload: Value = row.try_get("payload")?;
        let available_at: DateTime<Utc> = row.try_get("available_at")?;

        let _message_id = queue
            .publish(&topic, payload, available_at)
            .await
            .with_context(|| format!("publish outbox_id={outbox_id} to queue={topic}"))?;

        sqlx::query(
            r#"
            UPDATE state.outbox
            SET status = 'sent',
                updated_at = now()
            WHERE outbox_id = $1
            "#,
        )
        .bind(outbox_id)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("mark outbox_id={outbox_id} sent"))?;
    }

    tx.commit().await.context("commit outbox drain")?;
    Ok(())
}

async fn lease_reaper_loop(
    state: Arc<AppState>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let interval = Duration::from_millis(state.cfg.lease_reaper_poll_ms);

    loop {
        if *shutdown_rx.borrow() {
            return Ok(());
        }

        if let Err(err) = reap_expired_leases_once(&state.pool, &state.cfg).await {
            tracing::warn!(
                event = "harness.dispatcher.lease_reaper.error",
                error = %err,
                "lease reaper error"
            );
        }

        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            _ = shutdown_rx.changed() => {}
        }
    }
}

async fn reap_expired_leases_once(pool: &PgPool, cfg: &DispatcherConfig) -> anyhow::Result<()> {
    let mut tx = pool.begin().await.context("begin lease reaper tx")?;

    let rows = sqlx::query(
        r#"
        SELECT task_id, attempt
        FROM state.tasks
        WHERE status = 'running'
          AND lease_expires_at IS NOT NULL
          AND lease_expires_at < now()
        LIMIT 50
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .fetch_all(&mut *tx)
    .await
    .context("select expired leases")?;

    for row in rows {
        let task_id: Uuid = row.try_get("task_id")?;
        let attempt: i64 = row.try_get("attempt")?;
        let new_attempt = attempt + 1;

        let updated = sqlx::query(
            r#"
            UPDATE state.tasks
            SET attempt = $2,
                status = 'queued',
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = now()
            WHERE task_id = $1
              AND attempt = $3
              AND status = 'running'
            "#,
        )
        .bind(task_id)
        .bind(new_attempt)
        .bind(attempt)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("mark task_id={task_id} for retry"))?;

        if updated.rows_affected() == 0 {
            continue;
        }

        let outbox_id = outbox_id_for_task_wakeup(task_id, new_attempt);
        let payload = serde_json::json!({ "task_id": task_id });
        sqlx::query(
            r#"
            INSERT INTO state.outbox (outbox_id, topic, payload, available_at)
            VALUES ($1, $2, $3, now())
            ON CONFLICT (outbox_id) DO NOTHING
            "#,
        )
        .bind(outbox_id)
        .bind(&cfg.task_wakeup_queue)
        .bind(payload)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("enqueue retry wakeup task_id={task_id} attempt={new_attempt}"))?;
    }

    tx.commit().await.context("commit lease reaper")?;
    Ok(())
}

type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: &'static str,
}

impl ApiError {
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

    fn conflict(message: &'static str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message,
        }
    }

    fn internal<E: std::fmt::Display>(err: E) -> Self {
        tracing::error!(
            event = "harness.dispatcher.internal_error",
            error = %err,
            "dispatcher internal error"
        );
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal error",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = Json(serde_json::json!({ "error": self.message }));
        (self.status, body).into_response()
    }
}

fn require_task_capability(
    signer: &dyn SignerTrait,
    headers: &HeaderMap,
    task_id: Uuid,
    attempt: i64,
) -> ApiResult<()> {
    let token = headers
        .get(TASK_CAPABILITY_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("missing capability token"))?;

    let claims = signer.verify_task_capability(token).map_err(|err| {
        tracing::warn!(
            event = "harness.dispatcher.capability.invalid",
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

    Ok(())
}
