# Query SQL Gating (Milestone 4)

Risk: Medium
Public surface: trace-core `query::validate_sql(sql: &str) -> anyhow::Result<()>`

Summary: Add a conservative SQL validator used by Query Service to fail-closed on unsafe SQL.

Plan:
- Implement a comment/string-aware lexer that allows a single SELECT/CTE and rejects forbidden keywords, URI/path literals, and multi-statement SQL.
- Add unit tests covering INSTALL/LOAD/ATTACH, file/URL strings, non-SELECT, and multi-statement cases.

Acceptance:
- `validate_sql` accepts `SELECT 1` and `WITH t AS (SELECT 1) SELECT * FROM t`.
- Negative tests verify rejection without logging raw SQL.

Reduction:
- No new dependencies; pure Rust scanning.
