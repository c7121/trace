use clap::Parser;

/// Harness configuration.
///
/// Idiomatic Rust notes:
/// - Prefer explicit types over loosely-typed maps for config.
/// - Parse once at startup; pass `&HarnessConfig` through.
/// - Avoid global mutable state.
///
/// Defaults match `harness/docker-compose.yml`.
#[derive(Parser, Debug, Clone)]
pub struct HarnessConfig {
    /// Postgres state DB connection string.
    #[arg(long, env = "STATE_DATABASE_URL", default_value = "postgres://trace:trace@localhost:5433/trace_state")]
    pub state_database_url: String,

    /// Postgres data DB connection string.
    #[arg(long, env = "DATA_DATABASE_URL", default_value = "postgres://trace:trace@localhost:5434/trace_data")]
    pub data_database_url: String,

    /// Dispatcher bind address.
    #[arg(long, env = "DISPATCHER_BIND", default_value = "127.0.0.1:8080")]
    pub dispatcher_bind: String,

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

impl HarnessConfig {
    /// Parse config from environment only (no CLI parsing).
    ///
    /// We intentionally parse from a single fake argv element so clap doesn't try to interpret
    /// the harness subcommand flags here.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self::parse_from(["trace-harness"]))
    }
}
