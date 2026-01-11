# Query Service Data Model

Schema mapping notes for Query Service tables.

> These tables live in **Postgres data**. `org_id`/`user_id`/`task_id` are **soft references** to entities in **Postgres state** (no cross-DB foreign keys).

Where to look:
- Columns: [data_schema.md](data_schema.md)
- Implemented DDL: `harness/migrations/data/`

## data.query_audit (implemented v1)

Dataset-level audit log for task-scoped queries (`POST /v1/task/query`).

- Canonical DDL: `harness/migrations/data/0002_query_audit.sql`
- Invariants:
  - MUST NOT store raw SQL (store hashes/metadata only).
  - `columns_accessed` may be `NULL` when column-level attribution is not possible.
- Indexes: `(org_id, query_time DESC)`, `(task_id, query_time DESC)`

## data.user_query_audit (implemented v1)

Dataset-level audit log for user-scoped queries (`POST /v1/query`).

- Canonical DDL: `harness/migrations/data/0003_user_query_audit.sql`
- Notes:
  - `user_sub` is the IdP subject (stable per user).
  - `columns_accessed` may be `NULL` when column-level attribution is not possible.
- Indexes: `(org_id, query_time DESC)`, `(user_sub, query_time DESC)`

## data.saved_queries (planned)

Saved named queries with visibility controls.

- Columns: [data_schema.md](data_schema.md)
- Related specs: [query_service_user_query.md](../../specs/query_service_user_query.md), [query_service_query_results.md](../../specs/query_service_query_results.md)
- PII notes: [pii.md](pii.md)

## data.query_results (planned)

Query execution results and exports.

- Columns: [data_schema.md](data_schema.md)
- Spec: [query_service_query_results.md](../../specs/query_service_query_results.md)

## Related

- [query_service.md](../containers/query_service.md) - container boundaries and dependencies
- [query_service_task_query.md](../../specs/query_service_task_query.md) - task query endpoint semantics
- [query_service_user_query.md](../../specs/query_service_user_query.md) - user query endpoint semantics
- [query_service_query_results.md](../../specs/query_service_query_results.md) - query results and exports contract
- [pii.md](pii.md) - visibility and audit rules
