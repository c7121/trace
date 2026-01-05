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

    /// MinIO/S3 endpoint for dataset manifests and Parquet objects (Lite mode).
    #[arg(long, env = "S3_ENDPOINT", default_value = "http://localhost:9000")]
    pub s3_endpoint: String,

    /// Max allowed size of a dataset manifest JSON document in bytes.
    ///
    /// Treat the manifest as untrusted input even if "produced by us".
    #[arg(long, env = "DATASET_MAX_MANIFEST_BYTES", default_value_t = 1_048_576)]
    pub dataset_max_manifest_bytes: usize,

    /// Max allowed number of Parquet objects referenced by a dataset manifest.
    #[arg(long, env = "DATASET_MAX_OBJECTS", default_value_t = 2_048)]
    pub dataset_max_objects: usize,

    /// Max allowed size of any single Parquet object in bytes.
    #[arg(long, env = "DATASET_MAX_OBJECT_BYTES", default_value_t = 268_435_456)]
    pub dataset_max_object_bytes: u64,

    /// Max total bytes downloaded per dataset attachment (sum of Parquet objects).
    #[arg(long, env = "DATASET_MAX_TOTAL_BYTES", default_value_t = 1_073_741_824)]
    pub dataset_max_total_bytes: u64,
}

impl std::fmt::Debug for QueryServiceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let task_capability_next_secret = self
            .task_capability_next_secret
            .as_deref()
            .map(|_| "<redacted>");
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
            .field("dataset_max_manifest_bytes", &self.dataset_max_manifest_bytes)
            .field("dataset_max_objects", &self.dataset_max_objects)
            .field("dataset_max_object_bytes", &self.dataset_max_object_bytes)
            .field("dataset_max_total_bytes", &self.dataset_max_total_bytes)
            .finish()
    }
}

impl QueryServiceConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self::parse_from(["trace-query-service"]))
    }
}
