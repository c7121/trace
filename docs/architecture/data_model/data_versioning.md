# Data versioning schema mapping

Schema mapping for the behavior contract in [data_versioning.md](../data_versioning.md).

Doc ownership: this document explains where the behavior is stored (tables and key columns). It does not define behavior.

Where to look:
- Columns: [state_schema.md](state_schema.md)
- Relationships: [erd_state.md](erd_state.md)
- Dataset generations: [orchestration.md](orchestration.md)

## Tables

- `partition_versions`: per-partition materialization metadata within a `dataset_version`
  - Primary key: `(dataset_uuid, dataset_version, partition_key)`
  - Key columns: `materialized_at` (defaults to `now()`), `config_hash`, `schema_hash`, `location`, `row_count`, `bytes`
  - `partition_key` encodes a block range using `[start, end)` semantics (end-exclusive)
- `dataset_cursors`: per-consumer high-water mark within a `dataset_version`
  - Primary key: `(dataset_uuid, dataset_version, job_id)`
  - Key columns: `cursor_column`, `cursor_value` (stored as text), `updated_at` (defaults to `now()`)
- `data_invalidations`: scoped reprocessing requests for a specific `{dataset_uuid, dataset_version}`
  - Primary key: `id` (defaults to `gen_random_uuid()`)
  - Key columns: `scope` (`partition` or `row_range`), `partition_key`, `row_filter` (JSONB), `reason`, `source_event`
  - `reason` examples: `reorg`, `correction`, `manual`, `schema_change`
  - Audit columns: `created_at` (defaults to `now()`), `processed_by`, `processed_at`
  - Index: `(dataset_uuid, dataset_version)` for rows where `processed_at` is NULL
  - Example `row_filter`: `{"block_number": {"gte": 995, "lt": 1006}}`

## Related

- [data_versioning.md](../data_versioning.md) - incremental processing behavior and invariants
- [ADR 0009](../../adr/0009-atomic-cutover-and-query-pinning.md) - cutover and query pinning
