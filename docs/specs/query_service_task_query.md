# Query Service: Task Query API

Status: Implemented
Owner: Platform
Last updated: 2026-01-11

## Summary
Define `POST /v1/task/query` for task-scoped, validated, read-only SQL against an authorized dataset.
This endpoint is a thin wrapper around `trace_core::query::validate_sql`, trusted dataset attach, and audit logging.

## Risk
High

This adds a new endpoint that executes untrusted SQL (security/trust-boundary change).

## Context

Query Service context and boundaries: [Query Service container](../architecture/containers/query_service.md).

The SQL gate is `trace_core::query::validate_sql` (spec: [query_sql_gating.md](query_sql_gating.md)).

## Goals
- Provide `POST /v1/task/query` for task-scoped reads.
- Enforce capability-token authn/authz (token must match `{task_id, attempt}`).
- Enforce `validate_sql` and run queries against Parquet datasets attached via a pinned storage reference carried in the task capability token.
- Emit a dataset-level query audit record (no raw SQL).

## Non-goals
- User query endpoint (`/v1/query`) is owned by [query_service_user_query.md](query_service_user_query.md).
- Dataset registry, pagination/caching/export (future shape: [query_service_query_results.md](query_service_query_results.md)).
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
  - Auth: verify `X-Trace-Task-Capability` (see [task_capability_tokens.md](../architecture/contracts/task_capability_tokens.md)).
  - Gate: call `validate_sql(sql)` on every request.
  - Execute:
    - Trusted attach: attach a pinned dataset version using a storage reference carried in the task capability token as a DuckDB relation (`dataset`).
      - Implementation note: attach as a TEMP VIEW over `read_parquet(...)` (do not materialize into a table) so Parquet predicate/projection pushdown is preserved.
      - Query Service MUST NOT fetch Parquet bytes itself (`ObjectStore.get_bytes`) or copy Parquet objects to local temp as the primary attach path. Parquet is scanned in-place by DuckDB.
    - Untrusted SQL: execute gated SQL against attached relations only.
      - DuckDB runtime hardening is required as defense-in-depth; see [query_sql_gating.md](query_sql_gating.md).
  - Audit: insert dataset-level audit row into Postgres data DB.

### Data flow and trust boundaries
- Untrusted input: request JSON (`sql`, `limit`, `dataset_id`) + task capability JWT.
- Validation points:
  - JWT signature + expiry + `{task_id, attempt}` match.
  - `validate_sql` fail-closed.
  - DuckDB: disable host filesystem access and lock configuration; writes prevented by `validate_sql` (DDL/DML rejected). If the dataset itself is remote (HTTP/S3), DuckDB must be allowed to perform those authorized reads.
- Sensitive data handling:
  - MUST NOT log raw SQL (only structured denial/execution outcomes).
- Failure modes (dataset attach):
  - Permanent: malformed manifest, exceeds size limits, structural violations.
  - Retryable: object store temporarily unavailable (network errors, server 5xx, missing objects).

## Contract requirements
- MUST require `X-Trace-Task-Capability` and reject missing/invalid tokens (401).
- MUST reject a valid token that does not match `{task_id, attempt}` (403).
- MUST reject a request whose `dataset_id` is not granted in the capability token (403).
- MUST reject if the dataset storage reference is missing or outside the token’s S3 read prefixes (fail-closed).
- MUST return 400 when `validate_sql` rejects.
- MUST clamp `limit` to `[1, inline_row_limit]` and return `truncated` when clipped (defaults: [operations.md](../architecture/operations.md)).
- MUST write an audit row on successful execution without storing raw SQL.

## Security considerations
- Primary control: `validate_sql` denylist + single-statement requirement.
- Defense-in-depth: DuckDB runtime hardening and egress restrictions; see [query_sql_gating.md](query_sql_gating.md) and [ADR 0002](../adr/0002-networking.md).
- Residual risk: denylist incompleteness; mitigated by runtime sandboxing and tests.

## High risk addendum
### Observability and operability
- Logs: structured allow/deny events; MUST NOT include raw SQL.
- Metrics (later): query count, deny count, errors, duration buckets.

### Rollout and rollback
- Rollout: deploy internally only; not routed via Gateway.
- Rollback: disable/scale-to-zero Query Service; no data-plane corruption expected.

## Reduction pass
- No exports, pagination, caching, or federation (future shape: [query_service_query_results.md](query_service_query_results.md)).
- No new dependencies in `trace-core`; Query Service keeps DuckDB and HTTP deps out of core.

## Acceptance criteria
- Integration tests prove:
  - auth required (401/403),
  - dataset grants enforced (403 when not granted),
  - `validate_sql` gate enforced for INSTALL/LOAD/ATTACH and external readers,
  - allowed `SELECT` executes against an attached `dataset` relation backed by a deterministic Parquet fixture dataset (3 deterministic rows),
  - manifest cannot reference Parquet outside authorized S3 prefixes (403),
  - audit row inserted with correct `{org_id, task_id, dataset_id}`.
