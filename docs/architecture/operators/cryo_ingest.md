# cryo_ingest

Archive historical onchain data using [Cryo](https://github.com/paradigmxyz/cryo).

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
- Writes Cryo-convention filenames: `{dataset}_{start}_{end}.parquet` (end is inclusive)

## Scaling

Each cryo worker is configured with its own RPC API key. To run concurrent backfills:
- Define a `worker_pools` entry with N slots (each slot maps `MONAD_RPC_KEY` → a distinct secret name)
- Configure the job with `scaling.worker_pool` and `scaling.max_concurrency: N`
- Dispatcher leases one slot per running task; effective concurrency is `min(max_concurrency, pool size)`

## Dependencies

- RPC provider credentials (each worker configured with its own secret)
- S3 write access to cold bucket

## Example DAG Config

```yaml
- name: cryo_backfill
  activation: reactive
  runtime: ecs_rust
  operator: cryo_ingest
  execution_strategy: PerPartition
  idle_timeout: 0
  inputs:
    - from: { job: backfill_request, output: 0 }
  config:
    chain_id: 10143
    datasets: [blocks, transactions, logs]
    rpc_pool: monad
  scaling:
    worker_pool: monad_rpc_keys
    max_concurrency: 20   # dispatcher limits parallel jobs
  outputs: 3
  update_strategy: replace
  timeout_seconds: 3600
```
