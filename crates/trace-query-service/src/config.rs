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
            .finish()
    }
}

impl QueryServiceConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self::parse_from(["trace-query-service"]))
    }
}
