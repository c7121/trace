# block_follower

Follow chain tip in real-time and maintain canonical blocks/logs in **hot storage** (Postgres).

Status: Planned

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_platform` |
| **Activation** | `source` |
| **Source Kind** | `always_on` |
| **Image** | `block_follower:latest` |

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `chain_id` | config | Target chain (e.g., 10143 for Monad) |
| `rpc_pool` | config | RPC provider pool to use |
| `start_block` | config | Required: starting block for cold start |
| `emit_strategy` | config | `per_update` (default) or `threshold` |
| `threshold_blocks` | config | If `emit_strategy=threshold`, emit after N new blocks |
| `bootstrap.reset_outputs` | bootstrap | Optional: if true, truncate owned hot tables and restart from `start_block` on explicit bootstrap (not on crash restarts) |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Blocks | `postgres://hot_blocks` | Rows |
| Logs | `postgres://hot_logs` | Rows |
| Dataset events | Dispatcher | `{dataset_uuid, dataset_version, cursor|partition_key}` |

## Semantics

- Tracks the canonical chain tip and fills gaps when the head jumps forward.
- On reorg, rewrites the affected `block_number` range in hot Postgres and emits a row-range invalidation to the Dispatcher (attempt-fenced) so downstream jobs can rematerialize only what changed. See [data_versioning.md](../../architecture/data_versioning.md#reorg-handling).

- **Reset (optional):** if deployed with `bootstrap.reset_outputs: true`, the job performs a one-time bootstrap reset: truncate owned hot tables and restart from `start_block`. This is an explicit rebuild mechanism and is not triggered by normal restarts.

**Retention note:** `block_follower` does **not** decide how long data stays in Postgres. Hot retention/compaction is defined by downstream jobs in the DAG (e.g., compaction/purge operators). The Dispatcher treats these as normal jobs and does not interpret chain finality or retention policies.

## Hot Postgres table expectations

Hot chain tables should be designed for frequent **range rewrites** (reorgs) and optional **range deletes** (post-compaction retention):

- **Optional:** range partitioning by `(chain_id, block_number)` can make reorg rewrites and retention cleanup cheaper, but is not required in v1.
- **Minimum indexes:**
  - `INDEX (chain_id, block_number)` for range scans and bounded deletes.
  - `UNIQUE (chain_id, block_hash)` (or equivalent) to prevent duplicates and to support tip continuity checks.

## Example DAG config

```yaml
- name: block_follower
  activation: source
  runtime: ecs_platform
  operator: block_follower
  source:
    kind: always_on
  # Optional: wipe job-owned outputs and restart from start_block (executed once at activation/deploy)
  # bootstrap:
  #   reset_outputs: true
  config:
    chain_id: 10143
    rpc_pool: monad
    start_block: 1000000
    emit_strategy: per_update  # or: threshold
    threshold_blocks: 100      # only if emit_strategy=threshold
  outputs: 2
  update_strategy: replace
  heartbeat_timeout_seconds: 60
```
