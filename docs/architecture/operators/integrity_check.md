# integrity_check

Verify cold storage integrity against canonical chain state.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | Rust |
| **Execution Strategy** | Bulk |
| **Image** | `integrity_check:latest` |

## Description

Validates that finalized data in cold storage (S3 Parquet) matches canonical chain. Catches any corruption, incomplete compaction, or rare post-finality reorgs (51% attacks). This is a defense-in-depth measure—realtime reorgs are handled by `block_follower`.

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

## Triggers

- **Cron**: Periodic checks (e.g., daily, weekly)
- **Manual**: On-demand verification after incidents

## Behavior

- Reads block hashes from cold storage (S3 Parquet)
- Fetches canonical hashes from RPC for same blocks
- Compares hashes:
  - Match: block is valid
  - Mismatch: flag for recompaction
- If issues found:
  - Logs affected block numbers
  - Triggers `parquet_compact` job for affected ranges
  - Alerts ops channel
- Supports sampling for large datasets
- Idempotent: safe to run repeatedly

## Scope

This operator targets **finalized data only**. It does not check hot storage (Postgres), which is handled by `block_follower`'s realtime reorg detection.

## Dependencies

- RPC provider access
- S3 read access to Parquet files
- Read access to hot/cold storage
- Write access to reorg_checks table

## Example DAG Config

```yaml
- name: reorg_check
  job_type: Check
  execution_strategy: Bulk
  runtime: Rust
  entrypoint: reorg_check
  config:
    chain_id: 10143
    check_depth: 100
    rpc_pool: monad
  input_datasets: [hot_blocks, cold_blocks]
  output_dataset: null
  timeout_seconds: 300
```
