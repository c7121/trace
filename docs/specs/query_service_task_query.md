# Query Service: Task Query API

Status: Implemented
Owner: agent
Last updated: 2026-01-04

## Summary
Implement a minimal Query Service binary that exposes only `POST /v1/task/query` for task-scoped SQL.
It is intentionally a thin wrapper around `trace_core::query::validate_sql`, DuckDB sandbox defaults, and tests.

## Risk
High

This adds a new endpoint that executes untrusted SQL (security/trust-boundary change).

## Context
`docs/architecture/containers/query_service.md` defines DuckDB sandboxing requirements and a task-scoped `/v1/task/query` endpoint.
The SQL gate is `trace_core::query::validate_sql` (spec: `docs/specs/query_sql_gating.md`).

## Goals
- Provide a runnable Query Service with one task-scoped endpoint.
- Enforce capability-token authn/authz (token must match `{task_id, attempt}`).
- Enforce `validate_sql` and run queries against a deterministic in-memory DuckDB fixture with external access disabled.
- Emit a dataset-level query audit record (no raw SQL).

## Non-goals
- User query endpoint (`/v1/query`), dataset registry, pagination/caching/export.
- Postgres/S3 federation in DuckDB (use an in-memory fixture dataset only).

## Public surface changes
- Endpoints/RPC:
  - Add `POST /v1/task/query` (task capability token auth)
- Persistence format/migration:
  - Add `data.query_audit` (dataset-level audit log)
- Entrypoint exports:
  - New binary crate `crates/trace-query-service`
- Intentionally not supported:
  - Any non-SELECT SQL, multi-statement batches, extension install/load, external readers

## Proposed design
### Responsibilities and boundaries
- `trace-core`: provides `query::validate_sql` and capability token claim types.
- Query Service:
  - Auth: verify `X-Trace-Task-Capability` (HS256 dev secret in Lite).
  - Gate: call `validate_sql(sql)` on every request.
  - Execute: run SQL in embedded DuckDB with locked-down runtime settings against an in-memory fixture table.
  - Audit: insert dataset-level audit row into Postgres data DB.

### Data flow and trust boundaries
- Untrusted input: request JSON (`sql`, `limit`, `dataset_id`) + task capability JWT.
- Validation points:
  - JWT signature + expiry + `{task_id, attempt}` match.
  - `validate_sql` fail-closed.
  - DuckDB: external access disabled; writes prevented by `validate_sql` (DDL/DML rejected).
- Sensitive data handling:
  - MUST NOT log raw SQL (only structured denial/execution outcomes).

## Contract requirements
- MUST require `X-Trace-Task-Capability` and reject missing/invalid tokens (401).
- MUST reject a valid token that does not match `{task_id, attempt}` (403).
- MUST reject a request whose `dataset_id` is not granted in the capability token (403).
- MUST return 400 when `validate_sql` rejects.
- MUST clamp `limit` to `[1, 10_000]` (default 1000) and return `truncated` when clipped.
- MUST write an audit row on successful execution without storing raw SQL.

## Security considerations
- Primary control: `validate_sql` denylist + single-statement requirement.
- Defense-in-depth: DuckDB `enable_external_access=false` plus no extension auto-install/load.
- Residual risk: denylist incompleteness; mitigated by runtime sandboxing and tests.

## High risk addendum
### Observability and operability
- Logs: structured allow/deny events; MUST NOT include raw SQL.
- Metrics (later): query count, deny count, errors, duration buckets.

### Rollout and rollback
- Rollout: deploy internally only; not routed via Gateway.
- Rollback: disable/scale-to-zero Query Service; no data-plane corruption expected.

## Reduction pass
- Single endpoint only; no user mode, no exports, no federation.
- No new dependencies in `trace-core`; Query Service keeps DuckDB/axum deps out of core.

## Acceptance criteria
- Integration tests prove:
  - auth required (401/403),
  - dataset grants enforced (403 when not granted),
  - `validate_sql` gate enforced for INSTALL/LOAD/ATTACH and external readers,
  - allowed `SELECT` executes against the `alerts_fixture` in-memory fixture table (3 deterministic rows),
  - audit row inserted with correct `{org_id, task_id, dataset_id}`.
