# Query Service Data Model

Canonical DDL for Query Service tables.

> These tables live in **Postgres data**. `org_id`/`user_id`/`task_id` are **soft references** to entities in **Postgres state** (no cross-DB foreign keys).


## saved_queries

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

## query_results

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

- [query_service.md](../containers/query_service.md) — query endpoint behavior and capabilities
- [pii.md](pii.md) — visibility and audit rules
