# Query SQL gating

Status: Accepted (v1)
Owner: Platform
Last updated: 2026-01-11

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
- Perfect SQL parsing.

## DuckDB runtime hardening (defense in depth)

The SQL gate is necessary but not sufficient. Query execution MUST fail closed if these controls cannot be applied.

Recommended baseline:
- Disable extension auto-install and auto-load: `SET autoinstall_known_extensions=false; SET autoload_known_extensions=false;`
- Lock configuration: `SET lock_configuration=true;`
- Disable host filesystem access: `SET disabled_filesystems='LocalFileSystem';`
- Attach datasets in trusted code only; do not allow untrusted SQL to attach arbitrary catalogs or filesystems.
- Constrain spill-to-disk:
  - Set DuckDB `temp_directory` to an isolated per-query directory with `0700` permissions.
  - Prefer tmpfs (`/dev/shm`) when available; otherwise use a dedicated `/tmp` subdirectory.
  - Delete spill directories after the query completes.
- Do not mount host filesystem paths into the Query Service container.
- If remote Parquet scans are enabled (HTTP/S3), restrict OS/container egress to only the configured object store endpoints (see [ADR 0002](../adr/0002-networking.md)).

Defaults and timeouts: [operations.md](../architecture/operations.md).

Reduction:
- No new dependencies; pure Rust scanning.
