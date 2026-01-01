# Data Versioning Data Model

Canonical DDL for tables that track incremental materialization and invalidation.

Note: `dataset_versions` (dataset generations) is defined in [orchestration.md](orchestration.md).

## partition_versions

```sql
CREATE TABLE partition_versions (
    dataset_uuid UUID NOT NULL REFERENCES datasets(id),
    dataset_version UUID NOT NULL REFERENCES dataset_versions(id),
    partition_key TEXT NOT NULL,      -- e.g., "1000000-1010000" (block ranges are inclusive)
    materialized_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    config_hash TEXT,                 -- job config at time of materialization
    schema_hash TEXT,                 -- data shape (columns, types)
    location TEXT,                    -- s3://bucket/path or Postgres data table/view
    row_count BIGINT,
    bytes BIGINT,
    PRIMARY KEY (dataset_uuid, dataset_version, partition_key)
);
```

## dataset_cursors

```sql
CREATE TABLE dataset_cursors (
    dataset_uuid UUID NOT NULL REFERENCES datasets(id),
    dataset_version UUID NOT NULL REFERENCES dataset_versions(id),
    job_id UUID NOT NULL REFERENCES jobs(id),
    cursor_column TEXT NOT NULL,      -- e.g., "block_number"
    cursor_value TEXT NOT NULL,       -- e.g., "1005000" (stored as text for flexibility)
    updated_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (dataset_uuid, dataset_version, job_id)
);
```

## data_invalidations

```sql
CREATE TABLE data_invalidations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dataset_uuid UUID NOT NULL REFERENCES datasets(id),
    dataset_version UUID NOT NULL REFERENCES dataset_versions(id),
    scope TEXT NOT NULL,              -- 'partition' | 'row_range'
    partition_key TEXT,               -- for scope='partition'
    row_filter JSONB,                 -- for scope='row_range', e.g., {"block_number": {"gte": 995, "lte": 1005}}
    reason TEXT NOT NULL,             -- 'reorg' | 'correction' | 'manual' | 'schema_change'
    source_event JSONB,               -- details (e.g., reorg info: old_tip, new_tip, fork_block)
    created_at TIMESTAMPTZ DEFAULT now(),
    processed_by UUID[],              -- job_ids that have processed this invalidation
    processed_at TIMESTAMPTZ
);

CREATE INDEX idx_invalidations_dataset ON data_invalidations(dataset_uuid, dataset_version) WHERE processed_at IS NULL;
```

## Related

- [data_versioning.md](../data_versioning.md) — incremental processing behavior
- [ADR 0009](../adr/0009-atomic-cutover-and-query-pinning.md) — cutover and query pinning
