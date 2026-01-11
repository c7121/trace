# parquet_compact

Compact **finalized** hot data from Postgres into cold Parquet partitions in S3.

Status: planned

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_platform` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerPartition |
| **Image** | `parquet_compact:latest` |

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `chain_id` | config | Target chain (used for S3 layout) |
| `dataset` | config | What to compact (blocks, logs, etc.) |
| `partition_key` | event | Block range to compact (e.g., `"1000000-1010000"`; end-exclusive) |
| `finality_depth_blocks` | config | Only compact blocks `<= tip - finality_depth_blocks` |
| `chunk_size` | config | Rows per Parquet file |
| `delete_after_compact` | config | If true, delete the compacted range from hot Postgres |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Compacted data | `s3://{bucket}/cold/{chain}/{dataset}/` | Parquet |
| Partition manifest | staging prefix | `_manifest.json` + `_SUCCESS` |

## Behavior

- **Finality is job-defined:** the Dispatcher does not interpret finality/retention. This operator enforces finality using `finality_depth_blocks`.
- **Idempotent per range:** safe to re-run the same `partition_key`; uses `update_strategy: replace`.
- **Commit protocol:** writes Parquet to a task staging prefix and finalizes a manifest/marker; the Dispatcher commits and then emits `{dataset_uuid, dataset_version, partition_key}`. See [data_versioning.md](../../architecture/data_versioning.md#replace-output-commit-protocol-s3--parquet).
- **Hot retention:** if `delete_after_compact=true`, delete is performed only after successful output commit, and should be bounded to the same block range.

## Notes

If `delete_after_compact=true`, the baseline cleanup method is a bounded range delete after output commit (for example `DELETE ... WHERE chain_id=? AND block_number >= start AND block_number < end`). This keeps the operator table-agnostic, but large deletes can create bloat - ensure autovacuum is tuned accordingly.

**Future optimization:** if hot tables are range-partitioned on `(chain_id, block_number)` with boundaries aligned to compaction ranges, cleanup can be implemented as partition drops instead of row deletes.

## Example DAG config

```yaml
- name: parquet_compact
  activation: reactive
  runtime: ecs_platform
  operator: parquet_compact
  execution_strategy: PerPartition
  inputs:
    - from: { job: block_range_aggregate, output: 0 }
  outputs: 1
  config:
    chain_id: 10143
    dataset: blocks
    finality_depth_blocks: 100
    chunk_size: 10000
    delete_after_compact: true
  update_strategy: replace
  timeout_seconds: 1800
```

## Related

- Replace output commit protocol: [data_versioning.md](../../architecture/data_versioning.md)
- Range manifest producers: [range_aggregator.md](range_aggregator.md)
