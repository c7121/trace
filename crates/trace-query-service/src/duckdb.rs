use anyhow::Context;
use duckdb::{Connection, Config};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::task::JoinHandle;
use uuid::Uuid;

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
    db_path: PathBuf,
}

impl DuckDbSandbox {
    pub fn new(db_path: PathBuf, fixture_rows: u32) -> anyhow::Result<Self> {
        let conn = Connection::open(&db_path).context("open duckdb (rw) for fixture init")?;
        conn.execute_batch("BEGIN;").context("begin fixture tx")?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS alerts (
              id BIGINT NOT NULL,
              message VARCHAR NOT NULL
            );
            "#,
        )
        .context("create fixture table")?;

        // Re-create fixture deterministically for test isolation.
        conn.execute_batch("DELETE FROM alerts;").context("clear fixture table")?;
        for i in 0..fixture_rows {
            let id: i64 = (i as i64) + 1;
            let message = format!("alert-{id}");
            conn.execute(
                "INSERT INTO alerts (id, message) VALUES (?, ?)",
                duckdb::params![id, message],
            )
            .context("insert fixture row")?;
        }
        conn.execute_batch("COMMIT;").context("commit fixture tx")?;

        Ok(Self { db_path })
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub async fn query(&self, sql: String, max_rows: usize) -> anyhow::Result<QueryResultSet> {
        let db_path = self.db_path.clone();
        let handle: JoinHandle<anyhow::Result<QueryResultSet>> = tokio::task::spawn_blocking(move || {
            let conn = open_readonly(&db_path).context("open duckdb (ro)")?;
            apply_hardening(&conn).context("apply duckdb hardening")?;
            run_query(&conn, &sql, max_rows).context("run query")
        });

        handle
            .await
            .context("join duckdb worker")?
            .context("duckdb query failed")
    }
}

fn open_readonly(db_path: &Path) -> anyhow::Result<Connection> {
    // Prefer read-only access mode as defense-in-depth.
    let config = Config::default()
        .access_mode(duckdb::AccessMode::ReadOnly)
        .context("set duckdb access mode")?;
    Connection::open_with_flags(db_path, config).context("open duckdb with config")
}

fn apply_hardening(conn: &Connection) -> anyhow::Result<()> {
    // Fail-closed: if we can't apply these settings, refuse to run queries.
    //
    // NOTE: We still rely on `trace_core::query::validate_sql` as the primary gate.
    conn.execute_batch(
        r#"
        SET enable_external_access=false;
        SET autoinstall_known_extensions=false;
        SET autoload_known_extensions=false;
        "#,
    )
    .context("set hardening defaults")?;

    Ok(())
}

fn run_query(conn: &Connection, sql: &str, max_rows: usize) -> anyhow::Result<QueryResultSet> {
    let mut stmt = conn.prepare(sql).context("prepare")?;
    let column_count = stmt.column_count();

    let mut columns = Vec::with_capacity(column_count);
    for idx in 0..column_count {
        let name = stmt
            .column_name(idx)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| format!("col_{idx}"));
        let decl = format!("{:?}", stmt.column_type(idx));
        columns.push(QueryColumn { name, r#type: decl });
    }

    let mut rows = Vec::new();
    let mut result_rows = stmt.query([]).context("query")?;
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
        duckdb::types::ValueRef::Interval { months, days, nanos } => Value::Object(
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

pub fn default_duckdb_path(path_override: Option<&str>) -> anyhow::Result<PathBuf> {
    if let Some(path) = path_override {
        return Ok(PathBuf::from(path));
    }

    let mut p = std::env::temp_dir();
    p.push(format!("trace_query_service_fixture_{}.duckdb", Uuid::new_v4()));
    Ok(p)
}
