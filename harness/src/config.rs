use clap::Parser;
use uuid::Uuid;

/// Harness configuration.
///
/// Idiomatic Rust notes:
/// - Prefer explicit types over loosely-typed maps for config.
/// - Parse once at startup; pass `&HarnessConfig` through.
/// - Avoid global mutable state.
///
/// Defaults match `harness/docker-compose.yml`.
#[derive(Parser, Clone)]
pub struct HarnessConfig {
    /// Postgres state DB connection string.
    #[arg(
        long,
        env = "STATE_DATABASE_URL",
        default_value = "postgres://trace:trace@localhost:5433/trace_state"
    )]
    pub state_database_url: String,

    /// Postgres data DB connection string.
    #[arg(
        long,
        env = "DATA_DATABASE_URL",
        default_value = "postgres://trace:trace@localhost:5434/trace_data"
    )]
    pub data_database_url: String,

    /// Dispatcher bind address.
    #[arg(long, env = "DISPATCHER_BIND", default_value = "127.0.0.1:8080")]
    pub dispatcher_bind: String,

    /// Dispatcher base URL (used by worker/sink HTTP clients).
    #[arg(long, env = "DISPATCHER_URL", default_value = "http://127.0.0.1:8080")]
    pub dispatcher_url: String,

    /// Task lease duration in seconds.
    #[arg(long, env = "LEASE_DURATION_SECS", default_value_t = 10)]
    pub lease_duration_secs: u64,

    /// Outbox poll interval in milliseconds.
    #[arg(long, env = "OUTBOX_POLL_MS", default_value_t = 200)]
    pub outbox_poll_ms: u64,

    /// Lease reaper poll interval in milliseconds.
    #[arg(long, env = "LEASE_REAPER_POLL_MS", default_value_t = 200)]
    pub lease_reaper_poll_ms: u64,

    /// Max outbox rows to drain per tick.
    #[arg(long, env = "OUTBOX_BATCH_SIZE", default_value_t = 50)]
    pub outbox_batch_size: i64,

    /// Queue name for task wake-up messages.
    #[arg(long, env = "TASK_WAKEUP_QUEUE", default_value = "task_wakeup")]
    pub task_wakeup_queue: String,

    /// Queue name for buffer pointer messages.
    #[arg(long, env = "BUFFER_QUEUE", default_value = "buffer_queue")]
    pub buffer_queue: String,

    /// Queue name for poison buffer pointer messages.
    #[arg(long, env = "BUFFER_QUEUE_DLQ", default_value = "buffer_queue_dlq")]
    pub buffer_queue_dlq: String,

    /// Org ID embedded into issued task capability tokens.
    #[arg(
        long,
        env = "ORG_ID",
        default_value = "00000000-0000-0000-0000-000000000001"
    )]
    pub org_id: Uuid,

    /// Task capability token issuer.
    #[arg(
        long,
        env = "TASK_CAPABILITY_ISS",
        default_value = "trace-harness-dispatcher"
    )]
    pub task_capability_iss: String,

    /// Task capability token audience.
    #[arg(long, env = "TASK_CAPABILITY_AUD", default_value = "trace.task")]
    pub task_capability_aud: String,

    /// Task capability token key id (`kid`).
    #[arg(long, env = "TASK_CAPABILITY_KID", default_value = "dev")]
    pub task_capability_kid: String,

    /// Task capability token HMAC secret (HS256; harness-only).
    #[arg(
        long,
        env = "TASK_CAPABILITY_SECRET",
        default_value = "trace-harness-dev-secret"
    )]
    pub task_capability_secret: String,

    /// Next task capability token key id (`kid`) accepted during overlap window (optional).
    #[arg(long, env = "TASK_CAPABILITY_NEXT_KID")]
    pub task_capability_next_kid: Option<String>,

    /// Next task capability token HMAC secret accepted during overlap window (optional).
    #[arg(long, env = "TASK_CAPABILITY_NEXT_SECRET")]
    pub task_capability_next_secret: Option<String>,

    /// Task capability token TTL in seconds.
    #[arg(long, env = "TASK_CAPABILITY_TTL_SECS", default_value_t = 300)]
    pub task_capability_ttl_secs: u64,

    /// Worker poll interval in milliseconds.
    #[arg(long, env = "WORKER_POLL_MS", default_value_t = 200)]
    pub worker_poll_ms: u64,

    /// Worker queue visibility timeout in seconds.
    #[arg(long, env = "WORKER_VISIBILITY_TIMEOUT_SECS", default_value_t = 30)]
    pub worker_visibility_timeout_secs: u64,

    /// Worker requeue delay on transient failures (milliseconds).
    #[arg(long, env = "WORKER_REQUEUE_DELAY_MS", default_value_t = 500)]
    pub worker_requeue_delay_ms: u64,

    /// Sink poll interval in milliseconds.
    #[arg(long, env = "SINK_POLL_MS", default_value_t = 200)]
    pub sink_poll_ms: u64,

    /// Sink queue visibility timeout in seconds.
    #[arg(long, env = "SINK_VISIBILITY_TIMEOUT_SECS", default_value_t = 30)]
    pub sink_visibility_timeout_secs: u64,

    /// Sink requeue delay on parse/schema errors (milliseconds).
    #[arg(long, env = "SINK_RETRY_DELAY_MS", default_value_t = 200)]
    pub sink_retry_delay_ms: u64,

    /// Max deliveries for poison buffer messages before DLQ.
    #[arg(long, env = "SINK_MAX_DELIVERIES", default_value_t = 3)]
    pub sink_max_deliveries: i32,

    /// MinIO/S3 endpoint (used later by the pointer-buffer artifacts).
    #[arg(long, env = "S3_ENDPOINT", default_value = "http://localhost:9000")]
    pub s3_endpoint: String,

    #[arg(long, env = "S3_BUCKET", default_value = "trace-harness")]
    pub s3_bucket: String,

    #[arg(long, env = "S3_ACCESS_KEY", default_value = "trace")]
    pub s3_access_key: String,

    #[arg(long, env = "S3_SECRET_KEY", default_value = "tracepassword")]
    pub s3_secret_key: String,

    #[arg(long, env = "S3_REGION", default_value = "us-east-1")]
    pub s3_region: String,
}

impl std::fmt::Debug for HarnessConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let task_capability_next_secret = self
            .task_capability_next_secret
            .as_deref()
            .map(|_| "<redacted>");
        f.debug_struct("HarnessConfig")
            .field("state_database_url", &"<redacted>")
            .field("data_database_url", &"<redacted>")
            .field("dispatcher_bind", &self.dispatcher_bind)
            .field("dispatcher_url", &self.dispatcher_url)
            .field("lease_duration_secs", &self.lease_duration_secs)
            .field("outbox_poll_ms", &self.outbox_poll_ms)
            .field("lease_reaper_poll_ms", &self.lease_reaper_poll_ms)
            .field("outbox_batch_size", &self.outbox_batch_size)
            .field("task_wakeup_queue", &self.task_wakeup_queue)
            .field("buffer_queue", &self.buffer_queue)
            .field("buffer_queue_dlq", &self.buffer_queue_dlq)
            .field("org_id", &self.org_id)
            .field("task_capability_iss", &self.task_capability_iss)
            .field("task_capability_aud", &self.task_capability_aud)
            .field("task_capability_kid", &self.task_capability_kid)
            .field("task_capability_secret", &"<redacted>")
            .field("task_capability_next_kid", &self.task_capability_next_kid)
            .field("task_capability_next_secret", &task_capability_next_secret)
            .field("task_capability_ttl_secs", &self.task_capability_ttl_secs)
            .field("worker_poll_ms", &self.worker_poll_ms)
            .field(
                "worker_visibility_timeout_secs",
                &self.worker_visibility_timeout_secs,
            )
            .field("worker_requeue_delay_ms", &self.worker_requeue_delay_ms)
            .field("sink_poll_ms", &self.sink_poll_ms)
            .field("sink_visibility_timeout_secs", &self.sink_visibility_timeout_secs)
            .field("sink_retry_delay_ms", &self.sink_retry_delay_ms)
            .field("sink_max_deliveries", &self.sink_max_deliveries)
            .field("s3_endpoint", &self.s3_endpoint)
            .field("s3_bucket", &self.s3_bucket)
            .field("s3_access_key", &"<redacted>")
            .field("s3_secret_key", &"<redacted>")
            .field("s3_region", &self.s3_region)
            .finish()
    }
}

impl HarnessConfig {
    /// Parse config from environment only (no CLI parsing).
    ///
    /// We intentionally parse from a single fake argv element so clap doesn't try to interpret
    /// the harness subcommand flags here.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self::parse_from(["trace-harness"]))
    }
}
