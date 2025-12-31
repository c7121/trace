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
  - S3: versioned prefixes (e.g., `.../org/{org_id}/dataset/{dataset_uuid}/version/{dataset_version}/...`)
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
- v1 retention is conservative: keep all prior `dataset_version`s until an admin explicitly purges them (no automatic GC).

## Schema Sketch (names only)

This ADR focuses on the *cutover/rollback model*, not full schema definitions.

**Deploy/versioning pointers:**
- `dag_versions` — immutable DAG definition versions (YAML hash)
- `dag_current_versions` — current `dag_version` serving reads per `(org_id, dag_name)`
- `dataset_versions` — per-dataset materialization generations (version-addressed storage locations)
- `dag_version_datasets` — per-`dag_version` pointer set mapping `dataset_uuid` → `dataset_version`

**Incremental processing within a `dataset_version`:**
- `partition_versions`
- `dataset_cursors`
- `data_invalidations`

## Open Questions

- How we represent “ready for cutover” for large rematerializations (per-dataset and per-DAG).
- How buffered Postgres datasets interact with versioned physical tables (naming + migration rules for `storage_location`).
