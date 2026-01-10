# Query Service Data Model

Schema notes for Query Service tables.

Canonical DDL lives in `harness/migrations/data/` (applied in order). SQL blocks in this document may describe future tables and should be treated as illustrative unless they explicitly reference a migration file.

> These tables live in **Postgres data**. `org_id`/`user_id`/`task_id` are **soft references** to entities in **Postgres state** (no cross-DB foreign keys).

## query_audit (implemented v1)

Dataset-level audit log for task-scoped queries (`POST /v1/task/query`).

Notes:
- MUST NOT store raw SQL (store hashes/metadata only).
- `columns_accessed` may be `NULL` when column-level attribution is not possible.

```sql
-- Query audit log (dataset-level only; no raw SQL stored).

CREATE TABLE IF NOT EXISTS data.query_audit (
  id               BIGSERIAL PRIMARY KEY,
  org_id           UUID NOT NULL,
  task_id          UUID NOT NULL,
  dataset_id       UUID NOT NULL,
  query_time       TIMESTAMPTZ NOT NULL DEFAULT now(),
  columns_accessed JSONB NULL,
  result_row_count BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS query_audit_org_time_idx
  ON data.query_audit (org_id, query_time DESC);

CREATE INDEX IF NOT EXISTS query_audit_task_time_idx
  ON data.query_audit (task_id, query_time DESC);
```



## saved_queries (future)

```sql
CREATE TABLE saved_queries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL, -- soft ref: Postgres state orgs(id)
    user_id UUID NOT NULL, -- soft ref: Postgres state users(id)
    name TEXT NOT NULL,
    query TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'private',  -- see ../data_model/pii.md
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_saved_queries_org ON saved_queries(org_id);
```

## query_results (future)

```sql
CREATE TABLE query_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL, -- soft ref: Postgres state orgs(id)
    user_id UUID, -- soft ref: Postgres state users(id)
    mode TEXT NOT NULL,                -- 'interactive' | 'batch'
    status TEXT NOT NULL,              -- 'Queued' | 'Running' | 'Succeeded' | 'Failed'
    sql_hash TEXT NOT NULL,            -- hash only (avoid storing full SQL by default)
    saved_query_id UUID REFERENCES saved_queries(id),
    task_id UUID, -- soft ref: Postgres state tasks(id) (set for batch executions)
    output_format TEXT,                -- 'json' | 'csv' | 'parquet'
    output_location TEXT,              -- e.g., s3://bucket/results/{org_id}/{query_id}/
    row_count BIGINT,
    bytes BIGINT,
    duration_ms INT,
    error_code TEXT,
    error_message TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_query_results_org_time ON query_results(org_id, created_at DESC);
CREATE INDEX idx_query_results_user_time ON query_results(user_id, created_at DESC);
CREATE INDEX idx_query_results_task ON query_results(task_id);
```

## Related

- [query_service.md](../containers/query_service.md) - container boundaries and dependencies
- [query_service_task_query.md](../../specs/query_service_task_query.md) - task query endpoint semantics
- [query_service_user_query.md](../../specs/query_service_user_query.md) - user query endpoint semantics
- [query_service_query_results.md](../../specs/query_service_query_results.md) - query results and exports contract
- [pii.md](pii.md) - visibility and audit rules
