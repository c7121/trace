use clap::Parser;

/// Query Service configuration (Lite/harness defaults).
#[derive(Parser, Clone)]
pub struct QueryServiceConfig {
    /// Postgres data DB connection string (for audit logging).
    #[arg(
        long,
        env = "DATA_DATABASE_URL",
        default_value = "postgres://trace:trace@localhost:5434/trace_data"
    )]
    pub data_database_url: String,

    /// Bind address for the HTTP server.
    #[arg(long, env = "QUERY_SERVICE_BIND", default_value = "127.0.0.1:8090")]
    pub bind: String,

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

    /// Task capability token HMAC secret (HS256; lite-only).
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

    /// MinIO/S3 endpoint for Parquet dataset objects (Lite mode).
    #[arg(long, env = "S3_ENDPOINT", default_value = "http://localhost:9000")]
    pub s3_endpoint: String,

    /// Max bytes allowed for the dataset manifest (`_manifest.json`).
    #[arg(
        long,
        env = "QUERY_SERVICE_MAX_MANIFEST_BYTES",
        default_value_t = 1_048_576
    )]
    pub max_manifest_bytes: usize,

    /// Max parquet objects allowed in the dataset manifest.
    #[arg(
        long,
        env = "QUERY_SERVICE_MAX_MANIFEST_OBJECTS",
        default_value_t = 1024
    )]
    pub max_manifest_objects: usize,

    /// S3 access key for DuckDB httpfs S3 access (Lite mode; defaults match `harness/docker-compose.yml`).
    #[arg(long, env = "S3_ACCESS_KEY", default_value = "trace")]
    pub s3_access_key: String,

    /// S3 secret key for DuckDB httpfs S3 access (Lite mode).
    #[arg(long, env = "S3_SECRET_KEY", default_value = "tracepassword")]
    pub s3_secret_key: String,

    /// S3 region for DuckDB httpfs S3 access (Lite mode).
    #[arg(long, env = "S3_REGION", default_value = "us-east-1")]
    pub s3_region: String,

    /// S3 URL style for DuckDB (`path` for MinIO; `vhost` for AWS).
    #[arg(long, env = "S3_URL_STYLE", default_value = "path")]
    pub s3_url_style: String,

    /// Allow local file-based dataset storage refs (`scheme:"file"`) under `QUERY_SERVICE_LOCAL_FILE_ROOT`.
    #[arg(long, env = "QUERY_SERVICE_ALLOW_LOCAL_FILES", default_value_t = false)]
    pub allow_local_files: bool,

    /// Root directory allowed for local file dataset reads (required if local files are enabled).
    #[arg(long, env = "QUERY_SERVICE_LOCAL_FILE_ROOT")]
    pub local_file_root: Option<String>,
}

impl std::fmt::Debug for QueryServiceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let task_capability_next_secret = self
            .task_capability_next_secret
            .as_deref()
            .map(|_| "<redacted>");
        let s3_secret_key = "<redacted>";
        f.debug_struct("QueryServiceConfig")
            .field("data_database_url", &"<redacted>")
            .field("bind", &self.bind)
            .field("task_capability_iss", &self.task_capability_iss)
            .field("task_capability_aud", &self.task_capability_aud)
            .field("task_capability_kid", &self.task_capability_kid)
            .field("task_capability_secret", &"<redacted>")
            .field("task_capability_next_kid", &self.task_capability_next_kid)
            .field("task_capability_next_secret", &task_capability_next_secret)
            .field("task_capability_ttl_secs", &self.task_capability_ttl_secs)
            .field("s3_endpoint", &self.s3_endpoint)
            .field("max_manifest_bytes", &self.max_manifest_bytes)
            .field("max_manifest_objects", &self.max_manifest_objects)
            .field("s3_access_key", &"<redacted>")
            .field("s3_secret_key", &s3_secret_key)
            .field("s3_region", &self.s3_region)
            .field("s3_url_style", &self.s3_url_style)
            .field("allow_local_files", &self.allow_local_files)
            .field("local_file_root", &self.local_file_root)
            .finish()
    }
}

impl QueryServiceConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self::parse_from(["trace-query-service"]))
    }
}
