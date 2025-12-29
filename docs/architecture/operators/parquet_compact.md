# parquet_compact

Compact hot storage data into cold Parquet files.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_rust` |
| **Activation** | `reactive` |
| **Execution Strategy** | Bulk |
| **Image** | `parquet_compact:latest` |

## Description

Reads accumulated data from hot storage (Postgres) and writes optimized Parquet files to cold storage (S3). Handles partitioning, compression, and cleanup of compacted hot data.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `chain_id` | config | Target chain (used for S3 layout) |
| `dataset` | config | Which dataset to compact (blocks, logs, etc.) |
| `start_block` | config | First block in range |
| `end_block` | config | Last block in range |
| `chunk_size` | config | Rows per Parquet file |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Compacted data | `s3://{bucket}/cold/{chain}/{dataset}/` | Parquet |
| Manifest | `s3://{bucket}/cold/{chain}/{dataset}/manifest.json` | JSON |

## Execution

- **Threshold**: When hot storage reaches N blocks (from block_follower)
- **Cron**: Scheduled compaction runs
- **Manual**: On-demand compaction

## Behavior

- Reads from Postgres hot tables
- **Only compacts finalized blocks** — waits for finality threshold before compacting
- Writes Parquet with snappy compression
- Partitions by block-number range (partition_key `{start}-{end}` inclusive)
- Uses Cryo-style prefixes: `{dataset}_{start}_{end}*.parquet` (may emit multiple files per range due to `chunk_size`)
- Optionally deletes compacted rows from hot storage
- Idempotent: safe to re-run for same range
- Uses `update_strategy: replace` so reruns overwrite the same partition (used for repair/recompaction)

### Finality

- Finality threshold is chain-specific (e.g., 100 blocks for Monad)
- Only blocks older than `tip - finality_threshold` are compacted
- Ensures cold storage never contains reorg-able data

## Dependencies

- Postgres read access to hot tables
- S3 write access to cold bucket
- Postgres write access (if deleting compacted data)

## Example DAG Config

```yaml
- name: parquet_compact
  activation: reactive
  runtime: ecs_rust
  operator: parquet_compact
  execution_strategy: Bulk
  config:
    chain_id: 10143
    dataset: blocks
    chunk_size: 10000
    delete_after_compact: true
  input_datasets: [hot_blocks]
  output_datasets: [cold_blocks]
  update_strategy: replace
  timeout_seconds: 1800
```
