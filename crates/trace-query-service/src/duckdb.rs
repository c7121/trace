use anyhow::Context;
use crate::config::QueryServiceConfig;
use duckdb::{Config, Connection};
use serde_json::Value;
use tokio::task::JoinHandle;
use trace_core::DatasetStorageRef;

#[derive(Debug)]
pub enum DuckDbQueryError {
    Attach(anyhow::Error),
    Query(anyhow::Error),
}

impl std::fmt::Display for DuckDbQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DuckDbQueryError::Attach(err) => write!(f, "duckdb attach failed: {err}"),
            DuckDbQueryError::Query(err) => write!(f, "duckdb query failed: {err}"),
        }
    }
}

impl std::error::Error for DuckDbQueryError {}

#[derive(Debug, Clone)]
pub struct QueryColumn {
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone)]
pub struct QueryResultSet {
    pub columns: Vec<QueryColumn>,
    pub rows: Vec<Vec<Value>>,
}

#[derive(Clone)]
pub struct DuckDbSandbox {
    _private: (),
}

impl DuckDbSandbox {
    pub fn new() -> Self {
        Self { _private: () }
    }

    pub async fn query(&self, sql: String, max_rows: usize) -> anyhow::Result<QueryResultSet> {
        let handle: JoinHandle<anyhow::Result<QueryResultSet>> =
            tokio::task::spawn_blocking(move || {
                let conn = open_in_memory(false).context("open duckdb in-memory")?;
                apply_hardening(&conn).context("apply duckdb hardening")?;
                lock_down_local_filesystem(&conn).context("lock down local filesystem")?;
                run_query(&conn, &sql, max_rows).context("run query")
            });

        handle
            .await
            .context("join duckdb worker")?
            .context("duckdb query failed")
    }

    pub async fn query_with_dataset_storage_ref(
        &self,
        cfg: &QueryServiceConfig,
        storage_ref: &DatasetStorageRef,
        sql: String,
        max_rows: usize,
    ) -> Result<QueryResultSet, DuckDbQueryError> {
        let cfg = cfg.clone();
        let storage_ref = storage_ref.clone();
        let handle: JoinHandle<Result<QueryResultSet, DuckDbQueryError>> =
            tokio::task::spawn_blocking(move || {
                let conn = open_in_memory(true)
                    .context("open duckdb in-memory")
                    .map_err(DuckDbQueryError::Attach)?;
                apply_hardening(&conn)
                    .context("apply duckdb hardening")
                    .map_err(DuckDbQueryError::Attach)?;

                match storage_ref {
                    DatasetStorageRef::S3 { bucket, prefix, glob } => {
                        // Attach the dataset as a TEMP VIEW over remote Parquet so DuckDB can apply
                        // Parquet predicate/projection pushdown. This requires `httpfs` and network
                        // access; the security model relies on:
                        // - `trace_core::query::validate_sql` (untrusted SQL gate)
                        // - dataset grants from the task capability token (authz)
                        // - OS/container egress controls (allowlist object store endpoints)
                        (|| -> anyhow::Result<()> {
                            load_parquet(&conn).context("load parquet")?;
                            load_httpfs(&conn).context("load httpfs")?;
                            configure_s3(&conn, &cfg).context("configure s3")?;
                            lock_down_local_filesystem(&conn)
                                .context("lock down local filesystem")?;

                            let scan = s3_scan_target(&bucket, &prefix, &glob)?;
                            attach_parquet_dataset_view(&conn, &scan)
                                .context("attach parquet dataset")?;
                            Ok(())
                        })()
                        .map_err(DuckDbQueryError::Attach)?;
                    }
                    DatasetStorageRef::File { prefix, glob } => {
                        // Lite-only: allow local file scans when explicitly configured by the
                        // Query Service. Query Service enforces an allowlisted root dir; SQL is
                        // still gated by `validate_sql`.
                        (|| -> anyhow::Result<()> {
                            load_parquet(&conn).context("load parquet")?;
                            lock_configuration(&conn).context("lock configuration")?;

                            let scan = file_scan_target(&prefix, &glob)?;
                            attach_parquet_dataset_view(&conn, &scan)
                                .context("attach parquet dataset")?;
                            Ok(())
                        })()
                        .map_err(DuckDbQueryError::Attach)?;
                    }
                };

                run_query(&conn, &sql, max_rows)
                    .context("run query")
                    .map_err(DuckDbQueryError::Query)
            });

        handle
            .await
            .map_err(|err| {
                DuckDbQueryError::Query(anyhow::Error::new(err).context("join duckdb worker"))
            })?
    }
}

fn apply_hardening(conn: &Connection) -> anyhow::Result<()> {
    // Fail-closed: if we can't apply these settings, refuse to run queries.
    //
    // NOTE: We still rely on `trace_core::query::validate_sql` as the primary gate.
    conn.execute_batch(
        r#"
        SET autoinstall_known_extensions=false;
        SET autoload_known_extensions=false;
        "#,
    )
    .context("set hardening defaults")?;

    Ok(())
}

fn open_in_memory(external_access: bool) -> anyhow::Result<Connection> {
    let config = Config::default()
        .enable_autoload_extension(false)
        .context("disable extension autoload")?
        .enable_external_access(external_access)
        .context("set external access")?;

    Connection::open_in_memory_with_flags(config).context("open in-memory connection")
}

fn lock_down_local_filesystem(conn: &Connection) -> anyhow::Result<()> {
    // DuckDB hardening: disallow the LocalFileSystem so untrusted SQL cannot access the host
    // filesystem even if the SQL gate misses a file-reading function.
    //
    conn.execute_batch(
        "SET disabled_filesystems='LocalFileSystem';\nSET lock_configuration=true;",
    )
    .context("disable local filesystem")?;
    Ok(())
}

fn load_httpfs(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch("LOAD httpfs;").context("LOAD httpfs")?;
    Ok(())
}

fn load_parquet(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch("LOAD parquet;").context("LOAD parquet")?;
    Ok(())
}

fn configure_s3(conn: &Connection, cfg: &QueryServiceConfig) -> anyhow::Result<()> {
    let (endpoint, use_ssl) = normalize_s3_endpoint(&cfg.s3_endpoint)?;
    let endpoint = escape_sql_string(&endpoint);
    let region = escape_sql_string(&cfg.s3_region);
    let url_style = escape_sql_string(&cfg.s3_url_style);
    let access_key = escape_sql_string(&cfg.s3_access_key);
    let secret_key = escape_sql_string(&cfg.s3_secret_key);
    let use_ssl = if use_ssl { "true" } else { "false" };

    conn.execute_batch(&format!(
        r#"
        SET s3_endpoint='{endpoint}';
        SET s3_region='{region}';
        SET s3_url_style='{url_style}';
        SET s3_access_key_id='{access_key}';
        SET s3_secret_access_key='{secret_key}';
        SET s3_use_ssl={use_ssl};
        "#
    ))
    .context("configure duckdb s3")?;
    Ok(())
}

fn normalize_s3_endpoint(endpoint: &str) -> anyhow::Result<(String, bool)> {
    let mut s = endpoint.trim().trim_end_matches('/').to_string();
    let mut use_ssl = false;

    if let Some(rest) = s.strip_prefix("http://") {
        s = rest.to_string();
        use_ssl = false;
    } else if let Some(rest) = s.strip_prefix("https://") {
        s = rest.to_string();
        use_ssl = true;
    }

    // Drop any path component.
    if let Some((host, _path)) = s.split_once('/') {
        s = host.to_string();
    }

    if s.is_empty() {
        anyhow::bail!("empty s3 endpoint");
    }

    Ok((s, use_ssl))
}

fn lock_configuration(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch("SET lock_configuration=true;")
        .context("lock configuration")?;
    Ok(())
}

fn s3_scan_target(bucket: &str, prefix: &str, glob: &str) -> anyhow::Result<String> {
    if bucket.is_empty() {
        anyhow::bail!("empty s3 bucket");
    }
    let prefix = prefix.trim_start_matches('/');
    let prefix = if prefix.ends_with('/') {
        prefix.to_string()
    } else {
        format!("{prefix}/")
    };
    Ok(format!("s3://{bucket}/{prefix}{glob}"))
}

fn file_scan_target(prefix: &str, glob: &str) -> anyhow::Result<String> {
    let prefix = prefix.trim_end_matches('/');
    Ok(format!("{prefix}/{glob}"))
}

fn escape_sql_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn attach_parquet_dataset_view(conn: &Connection, scan: &str) -> anyhow::Result<()> {
    let scan = escape_sql_string(scan);
    let create = format!(
        "CREATE OR REPLACE TEMP VIEW dataset AS SELECT * FROM read_parquet('{scan}');"
    );
    conn.execute_batch(&create)
        .context("create temp dataset view")?;
    Ok(())
}

fn run_query(conn: &Connection, sql: &str, max_rows: usize) -> anyhow::Result<QueryResultSet> {
    let mut stmt = conn.prepare(sql).context("prepare")?;
    let mut rows = Vec::new();
    let mut result_rows = stmt.query([]).context("query")?;

    // NOTE: DuckDB statement schema is only available after execution.
    let stmt_ref = result_rows
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing statement metadata"))?;

    let column_count = stmt_ref.column_count();
    let mut columns = Vec::with_capacity(column_count);
    for idx in 0..column_count {
        let name = stmt_ref
            .column_name(idx)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| format!("col_{idx}"));
        let decl = format!("{:?}", stmt_ref.column_type(idx));
        columns.push(QueryColumn { name, r#type: decl });
    }

    while let Some(row) = result_rows.next().context("next row")? {
        let mut out = Vec::with_capacity(column_count);
        for idx in 0..column_count {
            let v = row.get_ref(idx).context("get column")?;
            out.push(value_ref_to_json(v));
        }
        rows.push(out);
        if rows.len() >= max_rows {
            break;
        }
    }

    Ok(QueryResultSet { columns, rows })
}

fn value_ref_to_json(value: duckdb::types::ValueRef<'_>) -> Value {
    match value {
        duckdb::types::ValueRef::Null => Value::Null,
        duckdb::types::ValueRef::Boolean(b) => Value::Bool(b),
        duckdb::types::ValueRef::TinyInt(i) => Value::Number((i as i64).into()),
        duckdb::types::ValueRef::SmallInt(i) => Value::Number((i as i64).into()),
        duckdb::types::ValueRef::Int(i) => Value::Number((i as i64).into()),
        duckdb::types::ValueRef::BigInt(i) => Value::Number(i.into()),
        duckdb::types::ValueRef::HugeInt(i) => Value::String(i.to_string()),
        duckdb::types::ValueRef::UTinyInt(i) => Value::Number((i as i64).into()),
        duckdb::types::ValueRef::USmallInt(i) => Value::Number((i as i64).into()),
        duckdb::types::ValueRef::UInt(i) => Value::Number((i as i64).into()),
        duckdb::types::ValueRef::UBigInt(i) => Value::String(i.to_string()),
        duckdb::types::ValueRef::Float(f) => serde_json::Number::from_f64(f as f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        duckdb::types::ValueRef::Double(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        duckdb::types::ValueRef::Decimal(d) => Value::String(d.to_string()),
        duckdb::types::ValueRef::Timestamp(_, v) => Value::Number(v.into()),
        duckdb::types::ValueRef::Text(bytes) => {
            Value::String(String::from_utf8_lossy(bytes).into_owned())
        }
        duckdb::types::ValueRef::Blob(bytes) => Value::Array(
            bytes
                .iter()
                .map(|b| Value::Number((*b as i64).into()))
                .collect(),
        ),
        duckdb::types::ValueRef::Date32(v) => Value::Number((v as i64).into()),
        duckdb::types::ValueRef::Time64(_, v) => Value::Number(v.into()),
        duckdb::types::ValueRef::Interval {
            months,
            days,
            nanos,
        } => Value::Object(
            [
                ("months".to_string(), Value::Number((months as i64).into())),
                ("days".to_string(), Value::Number((days as i64).into())),
                ("nanos".to_string(), Value::Number(nanos.into())),
            ]
            .into_iter()
            .collect(),
        ),
        duckdb::types::ValueRef::List(_, _) => Value::String("<list>".to_string()),
        duckdb::types::ValueRef::Enum(_, _) => Value::String("<enum>".to_string()),
        _ => Value::String("<unsupported>".to_string()),
    }
}
