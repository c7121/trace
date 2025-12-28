# Query Service

Stateless service for interactive and batch SQL queries across hot and cold storage.

## Overview

| Property | Value |
|----------|-------|
| **Type** | Platform service (not a job) |
| **Runtime** | Rust + embedded DuckDB |
| **Deployment** | ECS Fargate, behind ALB |

## Description

Accepts SQL queries via REST API and executes against federated hot (Postgres) and cold
(S3 Parquet) storage using embedded DuckDB. Designed for interactive, ad-hoc exploration,
with a batch mode that enqueues a `query` job when limits are exceeded.

## Endpoint

```
POST /v1/query
Authorization: Bearer <token>
Content-Type: application/json
```

### Request

```json
{
  "sql": "SELECT * FROM transactions WHERE block_number > 1000000 LIMIT 100",
  "mode": "interactive",
  "format": "json",
  "timeout_seconds": 30
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `sql` | string | required | SQL query (SELECT only) |
| `mode` | string | `interactive` | `interactive` or `batch` |
| `format` | string | `json` | Response format: `json`, `csv`, `parquet` |
| `timeout_seconds` | int | 30 | Max execution time (capped at 30s) |

### Response (interactive, inline result)

For results ≤ 10,000 rows:

```json
{
  "columns": [
    {"name": "hash", "type": "varchar"},
    {"name": "block_number", "type": "bigint"},
    {"name": "value", "type": "hugeint"}
  ],
  "rows": [
    ["0xabc...", 1000001, "1000000000000000000"],
    ["0xdef...", 1000002, "2500000000000000000"]
  ],
  "row_count": 2,
  "truncated": false,
  "duration_ms": 245
}
```

### Response (interactive, large result)

For results > 10,000 rows, written to S3 and returned as presigned URL:

```json
{
  "result_url": "https://s3.../results/{org_id}/{query_id}.parquet?X-Amz-...",
  "format": "parquet",
  "row_count": 150000,
  "bytes": 12345678,
  "expires_at": "2025-12-27T12:00:00Z",
  "duration_ms": 8420
}
```

### Response (batch)

Returned when `mode: batch` is requested or when interactive limits are exceeded:

```json
{
  "mode": "batch",
  "job_id": "uuid",
  "reason": "query exceeds interactive limits",
  "output_path": "s3://bucket/results/{org_id}/{job_id}/"
}
```

### Error Response

```json
{
  "error": "Query timeout exceeded",
  "code": "QUERY_TIMEOUT",
  "detail": "Query did not complete within 30 seconds. Consider narrowing your query or using batch mode."
}
```

## Interactive Constraints

| Constraint | Value | Rationale |
|------------|-------|-----------|
| Statement type | SELECT only | Read-only access enforced |
| Timeout | 30 seconds max | Prevent resource hogging |
| Inline result limit | 10,000 rows | Larger results → S3 |
| Result expiry | 1 hour | Presigned URLs for large results |

## Read-Only Enforcement

Queries are enforced read-only via:

1. **DuckDB read-only mode** — connection opened with `AccessMode::ReadOnly`
2. **Postgres read-only user** — service connects with a user granted only `SELECT` permissions

This provides defense in depth without SQL parsing overhead.

## Org Isolation

- Bearer token resolved to `org_id` via IdP / auth service
- DuckDB attaches **per-org views** that filter underlying tables by `org_id`
- User cannot query other orgs' data

## Data Sources

| Source | Attachment | Access |
|--------|------------|--------|
| Hot storage | Postgres via `postgres_scanner` | Read-only user |
| Cold storage | S3 Parquet via `httpfs` / `parquet_scan` | IAM role with S3 read |

Virtual tables (e.g., `transactions`) unify hot and cold transparently.

## Authentication

1. Client sends `Authorization: Bearer <token>`
2. Service validates token with IdP (Cognito/SSO)
3. Extracts `org_id`, `user_id`, `role` from claims
4. Rejects if token invalid or expired

## Dependencies

- **IdP** — token validation
- **Postgres** — hot storage (read-only user)
- **S3** — cold storage reads, result writes
- **DuckDB extensions** — `postgres_scanner`, `httpfs`

## Observability

| Metric | Description |
|--------|-------------|
| `query_duration_ms` | Execution time histogram |
| `query_count` | Queries per org/user |
| `query_errors` | Failures by error code |
| `query_result_rows` | Rows returned histogram |
| `query_result_bytes` | Bytes written to S3 |

Logs include: query hash (not full SQL for PII), org_id, user_id, duration, row_count, error (if any).

## Batch Mode

Batch mode creates a `query` job using the same operator as the interactive path.
Results are written to S3; clients poll job status or use webhooks for completion.

## Earmarked: Rate Limiting

Per-org and per-user rate limits. Design TBD.
