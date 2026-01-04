# Query Service

Stateless service for interactive and batch SQL queries across hot and cold storage.

## Overview

| Property | Value |
|----------|-------|
| **Type** | Platform service (not a job) |
| **Runtime** | Rust + embedded DuckDB |
| **Deployment** | ECS Fargate, behind ALB |

## Component View

```mermaid
flowchart LR
    gateway["Gateway"]:::container
    workers["Workers"]:::container
    duckdb["DuckDB"]:::component
    postgres["Postgres data"]:::database
    s3["S3 Parquet"]:::database

    gateway -->|SQL query| duckdb
    workers -->|SQL query task scoped| duckdb
    duckdb -->|query| postgres
    duckdb -->|query| s3

    classDef component fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef container fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
```

## Description

Accepts SQL queries via REST API and executes against federated hot (Postgres data) and cold
(S3 Parquet) storage using embedded DuckDB. Designed for interactive exploration with a batch mode for heavy queries.

## Query capabilities

The supported SQL surface is intentionally constrained. Query execution is **fail-closed**:

- The SQL validator (`trace-core::query::validate_sql`) MUST accept the query, and
- the runtime hardening MUST prevent external I/O and mutation even if a query slips through.

Canonical gate spec and required tests live in: `docs/specs/query_sql_gating.md`.

At a minimum, Query Service supports:
- A single `SELECT` (including `WITH` / CTEs).
- Reads from published/pinned datasets (Postgres views and/or S3 Parquet manifests).
- Common relational operators: joins, filters, aggregates, and window functions (as permitted by the gate).

Explicitly not supported:
- Any DDL/DML or multi-statement SQL.
- Extension install/load/attach workflows.
- File/URL readers or anything that implies filesystem or network access.

## Endpoint

```
POST /v1/query
Authorization: Bearer <token>
Content-Type: application/json
```

### Task Query API (UDF)

Untrusted UDF tasks may issue ad-hoc SQL using a **capability token** (not a user Bearer token).

```
POST /v1/task/query
X-Trace-Task-Capability: <capability_token>
Content-Type: application/json
```

> Task-scoped endpoints (`/v1/task/*`) are **internal-only** and are not routed through the public Gateway.

The request/response shape is the same as `/v1/query`, but dataset exposure is strictly limited to the dataset versions enumerated in the capability token.

**Verification:** Query Service validates the capability token as a JWT (signature + expiry).
- It should cache the Dispatcher’s internal task-JWKS (e.g., `GET /internal/jwks/task`) and refresh on `kid` miss.
- Query Service does not call Dispatcher per request for authorization; the token contents are the authorization.

### Request

```json
{
  "sql": "SELECT * FROM transactions WHERE block_number > 1000000 LIMIT 100",
  "mode": "interactive",
  "format": "json",
  "timeout_seconds": 60
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `sql` | string | required | SQL query (SELECT only) |
| `mode` | string | `interactive` | `interactive` or `batch` |
| `format` | string | `json` | Response format: `json`, `csv` (inline when small), `parquet` (exported) |
| `timeout_seconds` | int | (see operations) | Max execution time (clamped; see `docs/standards/operations.md`) |

### Responses

Interactive queries return results in one of two ways:

- **Inline** for small results (bounded by `inline_row_limit` and `inline_byte_limit`).
- **Exported** to S3 for larger results (and always when `format: parquet`).

User queries (`/v1/query`) may include a presigned `result_url`. Task-scoped queries (`/v1/task/query`)
return `output_location` and should fetch results using scoped STS credentials from the Dispatcher credential minting.

Exported results are written to caller-scoped prefixes:

- `/v1/query`: org results prefix (e.g., `s3://.../results/{org_id}/{query_id}/`).
- `/v1/task/query`: task scratch/export prefix from the capability token (so the task can read it via the Dispatcher credential minting).

#### Response (interactive, inline)

```json
{
  "mode": "interactive",
  "query_id": "uuid",
  "columns": [
    {"name": "hash", "type": "varchar"},
    {"name": "block_number", "type": "bigint"}
  ],
  "rows": [
    ["0xabc...", 1000001],
    ["0xdef...", 1000002]
  ],
  "row_count": 2,
  "truncated": false,
  "duration_ms": 245
}
```

#### Response (interactive, exported)

```json
{
  "mode": "interactive",
  "query_id": "uuid",
  "output_location": "s3://bucket/results/{org_id}/{query_id}/",
  "format": "parquet",
  "row_count": 150000,
  "bytes": 12345678,
  "expires_at": "2025-12-27T12:00:00Z",
  "result_url": "https://s3.../results/{org_id}/{query_id}/result.parquet?X-Amz-..." 
}
```

> `result_url` is optional and intended for user queries. Task-scoped callers should use `output_location`.

#### Response (batch)

Returned when `mode: batch` is requested or when interactive limits are exceeded:

```json
{
  "mode": "batch",
  "query_id": "uuid",
  "task_id": "uuid",
  "reason": "query exceeds interactive limits",
  "output_location": "s3://bucket/results/{org_id}/{query_id}/"
}
```

### Error Response

```json
{
  "error": "Query timeout exceeded",
  "code": "QUERY_TIMEOUT",
  "detail": "Query did not complete within the configured timeout. Consider narrowing your query or using batch mode."
}
```

## Interactive Constraints

| Constraint | Value | Rationale |
|------------|-------|-----------|
| Statement type | SELECT only | Read-only access enforced |
| Timeout | 60s (`/v1/query`), 300s (`/v1/task/query`) | Prevent resource hogging; long work uses batch |
| Inline result limit | 10,000 rows | Larger results → S3 |
| Inline byte limit | 10 MB | Prevent oversized responses; larger results → S3 |
| Presigned URL expiry | 1 hour | User queries only; task callers use `output_location` + STS |

## Read-Only Enforcement

DuckDB is opened with `AccessMode::ReadOnly`. Any DDL or DML statements fail at the DuckDB layer.

## SQL sandboxing (required)

Before executing any query, Query Service MUST call the gate:
- `trace-core::query::validate_sql(sql)`

The canonical rules + deny cases are specified in `docs/specs/query_sql_gating.md`.


Read-only mode is necessary but not sufficient. Query Service MUST also prevent SQL from accessing
unintended data sources (filesystem/HTTP/arbitrary S3) or bypassing the authorized dataset catalog.

v1 requirements:
- Queries may reference only platform-attached relations for authorized datasets.
  - `/v1/query`: published datasets only.
  - `/v1/task/query`: dataset versions enumerated in the capability token only.
- External access MUST be disabled for untrusted SQL:
  - no filesystem reads/writes,
  - no HTTP/URL reads,
  - no user-supplied S3/URI reads,
  - no `ATTACH` with user-supplied connection strings,
  - no extension install/load (or enforce a strict allowlist of built-in extensions only).
- Reject anything other than a single `SELECT` statement (no multi-statement batches).

Verification (required):
- Negative tests must fail closed: `read_csv('file://...')`, `read_parquet('http://...')`, `ATTACH ...`,
  `INSTALL/LOAD ...`, and any non-authorized relation reference.
- Log sandbox denials as structured events **without** logging raw SQL.

## Access Control

Query Service supports two authn/authz modes:

1. **User queries** (`/v1/query`)
   - Authenticated with a user Bearer token.
   - Exposes only **published datasets** from the dataset registry (see [ADR 0008](../../adr/0008-dataset-registry-and-publishing.md)).
   - Enforces org isolation and dataset visibility.

2. **Task-scoped queries** (`/v1/task/query`)
   - Authenticated with a **task capability token** issued by Dispatcher.
   - Exposes only the dataset versions enumerated in the token (may include internal/unpublished versions referenced by the task’s input edges).

For Postgres data-backed datasets, Query Service uses a read-only Postgres user and views filter by `org_id`.


## Dataset resolution and pinning

- **User queries** resolve `dataset_name` through the dataset registry and the producer DAG’s current `dag_version` pointer.
- **Task-scoped queries** are already pinned by the capability token (it contains resolved dataset versions/locations).

Pinning is per-query:
- Postgres data reads run inside a single transaction snapshot (e.g., `REPEATABLE READ`).
- S3/Parquet reads use a fixed manifest/file list resolved at query start.

For deploy/rematerialize cutover and rollback semantics, see [ADR 0009](../../adr/0009-atomic-cutover-and-query-pinning.md).



## PII access auditing

Query Service MUST emit `pii_access_log` entries when a query touches a dataset that has PII-classified columns.

Constraints:
- Do **not** store raw SQL in the audit log.
- For arbitrary SQL, Query Service may not be able to reliably determine selected columns. In that case:
  - record dataset-level access,
  - set `column_name` to `NULL`.

Suggested minimum fields:
- `org_id` (deployment org)
- `user_id` for `/v1/query`, or `task_id` for `/v1/task/query`
- `dataset`
- `record_id = query_id`
- `action = 'read'`

See `docs/architecture/data_model/pii.md`.


## Observability

| Metric | Description |
|--------|-------------|
| `query_duration_ms` | Execution time histogram |
| `query_count` | Queries per org/user |
| `query_errors` | Failures by error code |
| `query_result_rows` | Rows returned histogram |
| `query_result_bytes` | Bytes written to S3 |

## Admission & Limits

> **v1 is single-tenant.** Limits protect the service from runaway queries, not tenants from each other. Per-org quotas and stricter isolation deferred to multi-tenant.

- Concurrency cap: small fixed pool (e.g., 3-5 interactive queries). Beyond cap, requests queue briefly; if queue exceeds depth/age, force `mode: batch`.
- Memory cap with spill: DuckDB spill-to-disk enabled; log spill events.
- Timeouts: follow `docs/standards/operations.md` (60s for `/v1/query`, 300s for `/v1/task/query`); long-running jobs go to batch.
- Metrics: emit queue depth, queue age p95, spill count, OOM/circuit trips, forced-batch count.

Logs include: query hash (not full SQL for PII), org_id, user_id, duration, row_count, error (if any).

## Batch Mode

Batch mode creates a `query` task using the same operator as the interactive path and records a `query_results` row.
Results are written to S3; clients poll task status or fetch `query_results` by `query_id`.



## Query Results

Query executions (interactive and batch) are recorded in a platform-managed table. See [ADR 0005](../../adr/0005-query-results.md).

`query_id` in API responses is `query_results.id`.
