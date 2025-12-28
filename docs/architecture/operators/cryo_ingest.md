# cryo_ingest

Archive historical on-chain data using [Cryo](https://github.com/paradigmxyz/cryo).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_rust` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerPartition |
| **Idle Timeout** | `0` (batch) |
| **Image** | `cryo_ingest:latest` |

## Description

Fetches historical blockchain data (blocks, transactions, logs, traces) from RPC providers and writes to S3 as Parquet files. Used for backfills and archival.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `chain_id` | config | Target chain (e.g., 10143 for Monad) |
| `start_block` | config | First block to fetch |
| `end_block` | config | Last block to fetch |
| `datasets` | config | Cryo dataset types (blocks, txs, logs, traces) |
| `rpc_pool` | config | RPC provider pool to use |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Chain data | `s3://{bucket}/cold/{chain}/{dataset}/` | Parquet |
| Manifest | `s3://{bucket}/cold/{chain}/{dataset}/manifest.json` | JSON |

## Execution

- **Threshold**: When hot storage reaches N blocks
- **Manual**: Backfill requests (manual source emits events)
- **Cron**: Scheduled archival runs (cron source emits events)

## Behavior

- Idempotent: re-running same range overwrites with identical data
- Respects RPC rate limits via key pool
- Writes Cryo-convention filenames: `{dataset}_{start}_{end}.parquet`

## Dependencies

- RPC provider credentials (injected by Worker wrapper from Secrets Manager)
- S3 write access to cold bucket

## Example DAG Config

```yaml
- name: cryo_backfill
  activation: reactive
  runtime: ecs_rust
  operator: cryo_ingest
  execution_strategy: PerPartition
  idle_timeout: 0
  input_datasets: [backfill_requests]
  config:
    chain_id: 10143
    datasets: [blocks, transactions, logs]
    rpc_pool: monad
  scaling:
    mode: backfill        # or 'steady' for single-partition
    max_concurrency: 20   # dispatcher limits parallel jobs
  output_dataset: cold_blocks
  timeout_seconds: 3600
```
