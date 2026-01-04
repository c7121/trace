# Query SQL gating

Status: Accepted (v1)
Owner: Platform
Last updated: 2026-01-03

Risk: Medium
Public surface: trace-core `query::validate_sql(sql: &str) -> trace_core::Result<()>`

Summary: Add a conservative SQL validator used by Query Service to fail-closed on unsafe SQL.

Plan:
- Implement a comment/string-aware lexer that allows a single SELECT/CTE and rejects:
  - forbidden keywords (DDL/DML, extension install/load, `PRAGMA`, etc.),
  - known unsafe function call sites (e.g. `read_csv(...)`, `parquet_scan(...)`),
  - non-standard string-literal relations (e.g. `FROM 'file.csv'`),
  - multi-statement SQL.
- Add unit tests covering:
  - INSTALL/LOAD/ATTACH,
  - forbidden external-reader functions,
  - non-SELECT + multi-statement,
  - string-literal relations.

Acceptance:
- `validate_sql` accepts `SELECT 1` and `WITH t AS (SELECT 1) SELECT * FROM t`.
- Negative tests verify rejection without logging raw SQL.

Non-goals:
- Perfect SQL parsing. This gate is intentionally conservative and SHOULD be paired with DuckDB runtime hardening:
  - disable external access,
  - no network egress,
  - no host filesystem mounts,
  - restricted catalog attachment.

Reduction:
- No new dependencies; pure Rust scanning.
