# integrity_check

Defense-in-depth verification and repair against canonical chain state.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_platform` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate |
| **Image** | `integrity_check:latest` |

## Description

Validates that finalized data in cold storage (S3 Parquet) matches the canonical chain. Intended to catch corruption, incomplete compaction, downtime gaps, or rare post-finality reorgs. This is not the primary tip reorg mechanism — realtime reorg reconciliation is handled by `block_follower`.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `chain_id` | config | Target chain |
| `check_range` | config | Block range to verify (e.g., last 30 days of finalized) |
| `rpc_pool` | config | RPC provider pool to use |
| `sample_rate` | config | Optional sampling (e.g., 1% of blocks) for efficiency |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Integrity alerts | Dispatcher | Job request (for recompaction) |
| Check results | `postgres://integrity_checks` | Rows |

## Execution

- **Periodic**: Triggered by a cron source job (e.g., daily/weekly)
- **Manual**: Triggered on-demand after incidents

## Behavior

- Reads block hashes from cold storage (S3 Parquet)
- Fetches canonical hashes from RPC for same blocks
- Compares hashes:
  - Match: block is valid
  - Mismatch: flag for recompaction
- If issues found:
  - Logs affected block numbers
  - Triggers `parquet_compact` for affected partitions (typically via a `data_invalidations` partition invalidation; see [data_versioning.md](../data_versioning.md#data-invalidations))
  - Alerts ops channel
- Supports sampling for large datasets
- Idempotent: safe to run repeatedly

## Scope

This operator targets **finalized data only**. It does not check hot storage (Postgres data), which is handled by `block_follower`'s realtime reorg detection.

## Dependencies

- RPC provider access
- S3 read access to Parquet files
- Read access to hot/cold storage
- Postgres write access to `integrity_checks`

## Example DAG Config

```yaml
- name: integrity_check
  activation: reactive
  runtime: ecs_platform
  operator: integrity_check
  execution_strategy: PerUpdate
  inputs:
    - from: { job: daily_trigger, output: 0 }
  outputs: 1
  config:
    chain_id: 10143
    check_range: last_30d_finalized
    rpc_pool: monad
    sample_rate: 0.01
  update_strategy: replace
  timeout_seconds: 300
```
