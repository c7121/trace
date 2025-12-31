# ADR 0009: Atomic Cutover/Rollback + Query Pinning

## Status
- Accepted (December 2025)

## Decision

- Deploy/rematerialize is **non-destructive**: build new data in parallel and keep old versions for rollback.
- Cutover is **atomic**: once the new version is ready, swap the “current” pointer(s) in a **single Postgres transaction**.
- Rollback is also **atomic**: restore the prior pointer set in a single transaction (fast rollback).
- Query execution is **pinned**:
  - At query start, Query Service resolves `dataset_name -> dataset_uuid -> current dataset_version` and pins that mapping for the duration of the query.
  - For Postgres reads, Query Service runs inside a single transaction snapshot (e.g., `REPEATABLE READ`) so the query is not a moving target.
  - For S3/Parquet reads, Query Service uses a fixed manifest/file list resolved at query start.

### Versioned datasets

- Each published dataset has a stable `dataset_uuid` (see ADR 0008).
- Materializations write into **version-addressed artifacts** (per-dataset `dataset_version`), so “old” and “new” can coexist:
  - S3: versioned prefixes (e.g., `.../{dataset_uuid}/{dataset_version}/...`)
  - Postgres: versioned physical tables/views (naming is an implementation detail)
- “Current” is an indirection, not an overwrite: reads resolve via `dataset_version`.

### Deploy lifecycle (control-plane)

- A DAG deploy creates a new `dag_version` (staging) and a candidate pointer set for the datasets it produces.
- Only after the staged rematerialization is complete do we advance the DAG’s active pointer set (atomic cutover).
- Progressive cutover (partial pointer swaps) is explicitly avoided in v1.

## Context

- DAG edits can require rematerialization of a downstream subgraph.
- We want to keep serving **stale-but-consistent** data while rebuilding, and then switch cleanly.
- We want a **fast rollback** path that does not require recomputation.
- Queries should be consistent and debuggable; users should not see “half old / half new” results.

## Why

- **Atomic cutover** prevents downstream corruption and avoids mixed-version graphs.
- **Non-destructive rematerialize** enables rollback and safer iteration.
- **Query pinning** ensures reproducible results and avoids confusing “moving target” reads.

## Consequences

- We need an explicit notion of dataset versions and “current” pointers.
- Deploy must track a per-dataset pointer set per `dag_version` so rollback can restore the exact prior mapping.
- Storage layout must support keeping multiple dataset versions concurrently (retention/GC policy is a follow-up).

## Schema Sketch (illustrative)

```sql
-- A versioned deploy of a DAG definition (YAML).
CREATE TABLE dag_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dag_name TEXT NOT NULL,
    yaml_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (dag_name, yaml_hash)
);

-- Which deploy is currently serving reads (per DAG).
CREATE TABLE dag_current_versions (
    dag_name TEXT PRIMARY KEY,
    dag_version_id UUID NOT NULL REFERENCES dag_versions(id),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- A per-dataset materialization “generation” (can be updated over time as new data arrives).
CREATE TABLE dataset_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(), -- dataset_version
    dataset_uuid UUID NOT NULL REFERENCES datasets(id),
    created_at TIMESTAMPTZ DEFAULT now(),
    storage_location TEXT NOT NULL,                -- version-addressed location
    schema_hash TEXT,
    UNIQUE (dataset_uuid, id)
);

-- The pointer set for a given deploy.
CREATE TABLE dag_version_datasets (
    dag_version_id UUID NOT NULL REFERENCES dag_versions(id),
    dataset_uuid UUID NOT NULL REFERENCES datasets(id),
    dataset_version UUID NOT NULL REFERENCES dataset_versions(id),
    PRIMARY KEY (dag_version_id, dataset_uuid)
);
```

## Open Questions

- Version retention/GC policy (how many prior versions to keep; who triggers cleanup).
- How we represent “ready for cutover” for large rematerializations (per-dataset and per-DAG).
- How buffered Postgres datasets interact with versioned tables (naming + migration rules).

