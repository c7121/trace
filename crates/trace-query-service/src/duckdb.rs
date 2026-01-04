use anyhow::{anyhow, Context};
use duckdb::Connection;
use serde_json::Value;
use std::sync::{Arc, Mutex};
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
    conn: Arc<Mutex<Connection>>,
}

impl DuckDbSandbox {
    pub fn new_in_memory_fixture() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory().context("open duckdb in-memory")?;
        init_fixture(&conn).context("init fixture dataset")?;
        apply_hardening(&conn).context("apply duckdb hardening")?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn query(&self, sql: String, max_rows: usize) -> anyhow::Result<QueryResultSet> {
        let conn = self.conn.clone();
        let handle: JoinHandle<anyhow::Result<QueryResultSet>> =
            tokio::task::spawn_blocking(move || {
                let guard = conn.lock().map_err(|_| anyhow!("duckdb lock poisoned"))?;
                run_query(&guard, &sql, max_rows).context("run query")
            });

        handle
            .await
            .context("join duckdb worker")?
            .context("duckdb query failed")
    }
}

fn init_fixture(conn: &Connection) -> anyhow::Result<()> {
    // Deterministic, in-memory fixture dataset (exactly 3 rows).
    conn.execute_batch(
        r#"
        BEGIN;
        CREATE TABLE alerts_fixture (
          alert_definition_id VARCHAR NOT NULL,
          dedupe_key          VARCHAR NOT NULL,
          event_time          TIMESTAMP NOT NULL,
          chain_id            BIGINT NOT NULL,
          block_number        BIGINT NOT NULL,
          block_hash          VARCHAR NOT NULL,
          tx_hash             VARCHAR NOT NULL,
          payload             VARCHAR NOT NULL
        );

        INSERT INTO alerts_fixture VALUES
          (
            'alert_definition_1',
            'dedupe-001',
            '2025-01-01T00:00:00Z',
            1,
            100,
            '0xblockhash001',
            '0xtxhash001',
            '{"severity":"low","msg":"fixture-1"}'
          ),
          (
            'alert_definition_1',
            'dedupe-002',
            '2025-01-01T00:00:01Z',
            1,
            101,
            '0xblockhash002',
            '0xtxhash002',
            '{"severity":"medium","msg":"fixture-2"}'
          ),
          (
            'alert_definition_2',
            'dedupe-003',
            '2025-01-01T00:00:02Z',
            10,
            202,
            '0xblockhash003',
            '0xtxhash003',
            '{"severity":"high","msg":"fixture-3"}'
          );
        COMMIT;
        "#,
    )
    .context("create fixture table and rows")?;

    Ok(())
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
