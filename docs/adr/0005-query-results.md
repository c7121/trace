# ADR 0005: Query Results Table (Platform-Managed)

## Status
- Accepted (December 2025)

## Decision
- Record interactive and batch query executions in a platform-managed Postgres data table: `query_results`.
- Store **hashes/metadata** (not full SQL) by default to reduce accidental PII retention; full SQL lives in `saved_queries` when explicitly saved.
- The Query Service and the `query` (DuckDB) operator both write/update `query_results`.

## Context
- Query execution needs an auditable record: who ran what, when, how long, where results live.
- The batch `query` operator already emits “query metadata” but the core schema did not define it.
- We want a single place for the UI/API to list and fetch query runs without scraping logs or task payloads.

## Why
- **Consistency**: aligns with alerting’s platform tables (`alert_events`, `alert_deliveries`).
- **Observability**: supports dashboards, per-org usage, and debugging.
- **Security/PII hygiene**: prefer `sql_hash` over storing arbitrary query text by default.

## Consequences
- DB migrations must create `query_results` as part of platform bootstrap.
- Query Service records large/async results (and optionally all interactive queries) in `query_results`.
- Batch query tasks update the corresponding `query_results` row on completion.

## Related

- Normative surface: [query_service_query_results.md](../specs/query_service_query_results.md)
- Query Service surface: [query_service_user_query.md](../specs/query_service_user_query.md)
- Query operator surface: [operators/duckdb_query.md](../specs/operators/duckdb_query.md)
- Data model mapping: [query_service.md](../architecture/data_model/query_service.md)
- Container context: [query_service.md](../architecture/containers/query_service.md)
