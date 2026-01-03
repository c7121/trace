# cryo_ingest

Bootstrap historical onchain data using [Cryo](https://github.com/paradigmxyz/cryo).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_platform` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerPartition |
| **Idle Timeout** | `0` (batch) |
| **Image** | `cryo_ingest:latest` |

## Description

### Parallelism and RPC throughput

- Use `scaling.max_concurrency` to cap in-flight partitions for this job.
- Use `config.rpc_pool` to select an RPC pool managed by the RPC Egress Gateway. Pools may include multiple provider URLs/keys; keys must not appear in DAG YAML.


Fetches historical blockchain data (blocks, transactions, logs, traces) from RPC providers and writes to S3 as Parquet files. Used for bootstrap sync and archival.

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
- **Manual**: Bootstrap range requests (manual source emits events)
- **Cron**: Scheduled archival runs (cron source emits events)

## Behavior

- Idempotent: re-running same range overwrites with identical data
- Writes Cryo-convention filenames: `{dataset}_{start}_{end}.parquet` (end is inclusive)

## Scaling

Concurrency is controlled via `scaling.max_concurrency`.

RPC credentials are **not** modeled as per-task or per-slot secrets in DAG YAML. Instead:

- The job selects `config.rpc_pool`.
- The **RPC Egress Gateway** owns key pooling, rotation, and rate limiting for that pool.
- Workers authenticate only to the RPC Egress Gateway, not directly to external RPC providers.

This removes the need for `worker_pools` and avoids per-slot task definition sprawl.


## Dependencies

- RPC provider credentials (each worker configured with its own secret)
- S3 write access to cold bucket

## Example DAG Config

```yaml
- name: cryo_backfill
  activation: reactive
  runtime: ecs_platform
  operator: cryo_ingest
  execution_strategy: PerPartition
  idle_timeout: 0
  inputs:
    - from: { job: range_request, output: 0 }
  config:
    chain_id: 10143
    datasets: [blocks, transactions, logs]
    rpc_pool: monad
  scaling:
    max_concurrency: 20   # dispatcher limits parallel jobs
  outputs: 3
  update_strategy: replace
  timeout_seconds: 3600
```
