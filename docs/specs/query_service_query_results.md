# Query Service: Query Results and Exports

Status: Draft
Owner: Platform
Last updated: 2026-01-10

## Summary
Define the future Query Service contract for large result sets: inline vs exported responses, batch mode, and how exports map to `query_results`.

## Risk
High

This expands public surface and persistence semantics for query execution results.

## Context
Today the Query Service endpoints return inline JSON results only:
- User queries: `docs/specs/query_service_user_query.md`
- Task-scoped queries: `docs/specs/query_service_task_query.md`

This document defines the future result handling contract without re-specifying authn/authz or SQL gating:
- SQL gate: `docs/specs/query_sql_gating.md`
- Token/claims: `docs/architecture/contracts/task_capability_tokens.md`

## Goals
- Keep small results inline for interactive use.
- Export large results to object storage and return an `output_location`.
- Support optional presigned fetch URLs for user queries.
- Support a batch mode that materializes results asynchronously.
- Record executions in `query_results` and treat `query_id` as the stable identifier.

## Non-goals
- Pagination
- Saved queries UI
- Cross-dataset attach in a single request

## Proposed contract

This doc describes the additional request and response fields for exports and batch mode. Endpoint-specific base fields stay owned by the endpoint specs.

### Request

```json
{
  "sql": "SELECT * FROM transactions WHERE block_number > 1000000 LIMIT 100",
  "mode": "interactive",
  "format": "json",
  "timeout_seconds": 60
}
```

Additional fields:

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `mode` | string | `interactive` | `interactive` or `batch` |
| `format` | string | `json` | `json`, `csv` (inline when small), `parquet` (exported) |
| `timeout_seconds` | int | (see operations) | Max execution time (clamped; see `docs/architecture/operations.md`) |

### Responses

Interactive queries return results in one of two ways:

- **Inline** for small results (bounded by `inline_row_limit` and `inline_byte_limit`, see `docs/architecture/operations.md`).
- **Exported** to object storage for larger results (and always when `format: parquet`).

Exported results are written to caller-scoped prefixes:

- `/v1/query`: org results prefix (example: `s3://.../results/{org_id}/{query_id}/`).
- `/v1/task/query`: task scratch or export prefix from the capability token.

User queries may include a presigned `result_url`. Task-scoped callers should fetch results using scoped credentials from Dispatcher credential minting and the returned `output_location`.

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

Notes:
- `result_url` is optional and intended for user queries.
- Task-scoped callers should rely on `output_location`.

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

### Error response

```json
{
  "error": "Query timeout exceeded",
  "code": "QUERY_TIMEOUT",
  "detail": "Query did not complete within the configured timeout. Consider narrowing your query or using batch mode."
}
```

## Interactive constraints

Interactive execution is bounded. Default values live in `docs/architecture/operations.md`.

| Constraint | Default | Rationale |
|------------|---------|-----------|
| Statement type | SELECT only | Read-only access enforced |
| Timeout | (see operations) | Prevent resource hogging; long work uses batch mode |
| Inline row limit | (see operations) | Prevent oversized responses; larger results export |
| Inline byte limit | (see operations) | Prevent oversized responses; larger results export |
| Presigned URL expiry | (see operations) | User query only; task callers use `output_location` and scoped credentials |

## Query results mapping

Query executions (interactive and batch) are recorded in a platform-managed Postgres data table: `query_results`.

Normative decision: `docs/adr/0005-query-results.md`.

Schema: `docs/architecture/data_model/query_service.md`.

Rules:
- `query_id` in API responses is `query_results.id`.
- Batch mode creates a `query` task using the same operator as the interactive path and associates it with the same `query_results` row via `task_id`.
- Clients may poll task status or fetch `query_results` by `query_id`.
