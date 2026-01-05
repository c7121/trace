# Query Service: Task Query API

Status: Implemented
Owner: agent
Last updated: 2026-01-05

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
- Enforce `validate_sql` and run queries against Parquet datasets attached via a pinned manifest referenced by the task capability token.
- Emit a dataset-level query audit record (no raw SQL).

## Non-goals
- User query endpoint (`/v1/query`), dataset registry, pagination/caching/export.
- General Postgres federation in DuckDB.
- Any dataset discovery/resolution beyond the dataset grants carried in the task capability token.

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
  - Execute:
    - Trusted attach: resolve a pinned dataset manifest from object storage (MinIO/S3) and attach it as a DuckDB relation (`dataset`).
      - Implementation note: attach as a TEMP VIEW over `read_parquet(...)` (do not materialize into a table) so Parquet predicate/projection pushdown is preserved.
    - Untrusted SQL: execute gated SQL against attached relations only.

      DuckDB runtime hardening MUST be applied in addition to SQL gating:
      - disable host filesystem access (e.g. `SET disabled_filesystems='LocalFileSystem'`),
      - lock configuration (`SET lock_configuration=true`),
      - disable extension auto-install (no `INSTALL` from untrusted SQL; `autoinstall_known_extensions=false`),
      - run in an OS/container sandbox with egress restricted to only the object-store endpoint(s).
  - Audit: insert dataset-level audit row into Postgres data DB.

### Data flow and trust boundaries
- Untrusted input: request JSON (`sql`, `limit`, `dataset_id`) + task capability JWT.
- Validation points:
  - JWT signature + expiry + `{task_id, attempt}` match.
  - `validate_sql` fail-closed.
  - DuckDB: disable host filesystem access and lock configuration; writes prevented by `validate_sql` (DDL/DML rejected). If the dataset itself is remote (HTTP/S3), DuckDB must be allowed to perform those authorized reads.
- Sensitive data handling:
  - MUST NOT log raw SQL (only structured denial/execution outcomes).

## Contract requirements
- MUST require `X-Trace-Task-Capability` and reject missing/invalid tokens (401).
- MUST reject a valid token that does not match `{task_id, attempt}` (403).
- MUST reject a request whose `dataset_id` is not granted in the capability token (403).
- MUST reject if the dataset storage reference is missing or outside the token’s S3 read prefixes (fail-closed).
- MUST return 400 when `validate_sql` rejects.
- MUST clamp `limit` to `[1, 10_000]` (default 1000) and return `truncated` when clipped.
- MUST write an audit row on successful execution without storing raw SQL.

## Security considerations
- Primary control: `validate_sql` denylist + single-statement requirement.
- Defense-in-depth: DuckDB runtime hardening (disable `LocalFileSystem`, lock configuration, no extension auto-install). If remote datasets are supported, pair with OS-level egress controls so DuckDB can only reach the configured object-store endpoint(s).
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
  - allowed `SELECT` executes against an attached `dataset` relation backed by a deterministic Parquet fixture dataset (3 deterministic rows),
  - manifest cannot reference Parquet outside authorized S3 prefixes (403),
  - audit row inserted with correct `{org_id, task_id, dataset_id}`.
