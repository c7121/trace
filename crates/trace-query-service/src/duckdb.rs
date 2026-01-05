use anyhow::Context;
use duckdb::Connection;
use serde_json::Value;
use tokio::task::JoinHandle;

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
                let conn = Connection::open_in_memory().context("open duckdb in-memory")?;
                apply_hardening(&conn).context("apply duckdb hardening")?;
                lock_down_local_filesystem(&conn).context("lock down local filesystem")?;
                lock_down_external_access(&conn).context("lock down external access")?;
                run_query(&conn, &sql, max_rows).context("run query")
            });

        handle
            .await
            .context("join duckdb worker")?
            .context("duckdb query failed")
    }

    pub async fn query_with_dataset_urls(
        &self,
        parquet_urls: &[String],
        sql: String,
        max_rows: usize,
    ) -> anyhow::Result<QueryResultSet> {
        let parquet_urls = parquet_urls.to_vec();
        let handle: JoinHandle<anyhow::Result<QueryResultSet>> =
            tokio::task::spawn_blocking(move || {
                let conn = Connection::open_in_memory().context("open duckdb in-memory")?;
                apply_hardening(&conn).context("apply duckdb hardening")?;

                // Attach the dataset using a TEMP VIEW so DuckDB can push down filters/projections
                // into the Parquet scan (including for remote HTTP/S3 reads).
                attach_parquet_dataset(&conn, &parquet_urls).context("attach parquet dataset")?;

                // Runtime hardening for untrusted SQL:
                // - disable host filesystem access (LocalFileSystem)
                // - lock config to prevent re-enabling
                //
                // NOTE: We intentionally do *not* set `enable_external_access=false` here because
                // the authorized dataset itself may require remote reads (HTTP range requests via
                // DuckDB's httpfs file system). Authorization is enforced via capability token
                // grants + manifest validation.
                lock_down_local_filesystem(&conn).context("lock down local filesystem")?;

                run_query(&conn, &sql, max_rows).context("run query")
            });

        handle
            .await
            .context("join duckdb worker")?
            .context("duckdb query failed")
    }
}

fn apply_hardening(conn: &Connection) -> anyhow::Result<()> {
    // Fail-closed: if we can't apply these settings, refuse to run queries.
    //
    // NOTE: We still rely on `trace_core::query::validate_sql` as the primary gate.
    conn.execute_batch(
        r#"
        SET autoinstall_known_extensions=false;
        "#,
    )
    .context("set hardening defaults")?;

    Ok(())
}

fn lock_down_external_access(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch("SET enable_external_access=false;")
        .context("disable external access")?;
    Ok(())
}

fn lock_down_local_filesystem(conn: &Connection) -> anyhow::Result<()> {
    // DuckDB hardening: disallow the LocalFileSystem so untrusted SQL cannot access the host
    // filesystem even if the SQL gate misses a file-reading function.
    //
    // We also lock configuration to prevent changes at runtime.
    conn.execute_batch(
        "SET disabled_filesystems='LocalFileSystem';\nSET lock_configuration=true;",
    )
    .context("disable local filesystem")?;
    Ok(())
}

fn attach_parquet_dataset(conn: &Connection, parquet_urls: &[String]) -> anyhow::Result<()> {
    if parquet_urls.is_empty() {
        return Err(anyhow::anyhow!("no parquet objects"));
    }

    // Enable remote file access for Parquet reads.
    //
    // NOTE: We intentionally do *not* run `INSTALL httpfs;` here. The service is expected to
    // run with DuckDB built/bundled with httpfs available locally.
    conn.execute_batch("LOAD httpfs;")
        .context("load httpfs extension")?;

    let scan = if parquet_urls.len() == 1 {
        let url = parquet_urls[0].replace('\'', "''");
        format!("read_parquet('{url}')")
    } else {
        let mut parts = Vec::with_capacity(parquet_urls.len());
        for url in parquet_urls {
            let url = url.replace('\'', "''");
            parts.push(format!("'{url}'"));
        }
        format!("read_parquet([{}])", parts.join(","))
    };

    // Use a VIEW to preserve Parquet pushdown.
    let create = format!("CREATE TEMP VIEW dataset AS SELECT * FROM {scan};");
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
        duckdb::types::ValueRef::Struct(_, _) => Value::String("<struct>".to_string()),
        duckdb::types::ValueRef::Array(_, _) => Value::String("<array>".to_string()),
        duckdb::types::ValueRef::Map(_, _) => Value::String("<map>".to_string()),
        duckdb::types::ValueRef::Union(_, _) => Value::String("<union>".to_string()),
    }
}
