# cryo_ingest

Bootstrap historical onchain data using [Cryo](https://github.com/paradigmxyz/cryo).

Status: Implemented (Lite/harness stub)

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
| Chain data | `s3://{bucket}/cryo/{chain_id}/{dataset}/{start_block}_{end_block}/{dataset_version}/...` | Parquet |

## Execution

- **Threshold**: When hot storage reaches N blocks
- **Manual**: Bootstrap range requests (manual source emits events)
- **Cron**: Scheduled archival runs (cron source emits events)

## Behavior

- Idempotent: re-running the same `{chain_id, range, config_hash}` produces the same deterministic `dataset_version` and `storage_prefix`
- Writes deterministic Parquet object keys that keep the range visible for debugging (end is inclusive).

### Lite/harness note
In the harness, `cryo_ingest` defaults to a deterministic stub that writes a small Parquet dataset without requiring the real Cryo binary or chain RPC access. Real Cryo integration is opt-in and introduced incrementally.

### Local staging (Lite)
When running Cryo locally (or in Lite mode), the worker MUST:
- write Cryo output only to a per-task staging directory (e.g. `/tmp/trace/cryo/<task_id>/<attempt>/`),
- delete the staging directory after upload-to-object-store succeeds,
- delete stale staging dirs older than N hours on worker startup (crash cleanup),
- ensure Query Service does not have access to the staging dir (do not mount it into the QS container).

## Scaling

Concurrency is controlled via `scaling.max_concurrency`.

RPC credentials are **not** modeled as per-task or per-slot secrets in DAG YAML. Instead:

- The job selects `config.rpc_pool`.
- The **RPC Egress Gateway** owns key pooling, rotation, and rate limiting for that pool.
- Workers authenticate only to the RPC Egress Gateway, not directly to external RPC providers.

This removes the need for `worker_pools` and avoids per-slot task definition sprawl.


## Dependencies

- An `rpc_pool` configured in the RPC Egress Gateway (provider endpoints + API keys live there, not in DAG YAML).
- S3 write access to the cold bucket for replace-style outputs.

## Example DAG Config

```yaml
- name: cryo_bootstrap
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

## Future work

- **Cryo as a Rust library:** Wrap Cryo as a library and plumb a custom writer/output abstraction so Parquet objects can be streamed directly to the configured object store (S3/MinIO) without local staging. Track this in `docs/plan/backlog.md`.
