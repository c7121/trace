# parquet_compact

Compact hot storage data into cold Parquet files.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | Rust |
| **Execution Strategy** | Bulk |
| **Image** | `parquet_compact:latest` |

## Description

Reads accumulated data from hot storage (Postgres) and writes optimized Parquet files to cold storage (S3). Handles partitioning, compression, and cleanup of compacted hot data.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `dataset` | config | Which dataset to compact (blocks, logs, etc.) |
| `start_block` | config | First block in range |
| `end_block` | config | Last block in range |
| `chunk_size` | config | Rows per Parquet file |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Compacted data | `s3://{bucket}/cold/{dataset}/` | Parquet |
| Manifest | `s3://{bucket}/cold/{dataset}/manifest.json` | JSON |

## Triggers

- **Threshold**: When hot storage reaches N blocks (from block_follower)
- **Cron**: Scheduled compaction runs
- **Manual**: On-demand compaction

## Behavior

- Reads from Postgres hot tables
- **Only compacts finalized blocks** — waits for finality threshold before compacting
- Writes Parquet with snappy compression
- Partitions by block range
- Optionally deletes compacted rows from hot storage
- Idempotent: safe to re-run for same range

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
  job_type: Transform
  execution_strategy: Bulk
  runtime: Rust
  entrypoint: parquet_compact
  config:
    dataset: blocks
    chunk_size: 10000
    delete_after_compact: true
  input_datasets: [hot_blocks]
  output_dataset: cold_blocks
  timeout_seconds: 1800
```
