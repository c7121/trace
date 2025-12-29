# block_follower

Follow chain tip in real-time, writing new blocks to hot storage.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_rust` |
| **Activation** | `source` |
| **Source Kind** | `always_on` |
| **Image** | `block_follower:latest` |

## Description

Long-running service that subscribes to new blocks at chain tip and writes them to hot storage (Postgres) immediately. Emits threshold events when block count reaches configurable limits.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `chain_id` | config | Target chain (e.g., 10143 for Monad) |
| `rpc_pool` | config | RPC provider pool to use |
| `threshold_blocks` | config | Emit event after N new blocks |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Block data | `postgres://hot_blocks` | Rows |
| Log data | `postgres://hot_logs` | Rows |
| Threshold events | Dispatcher | Event |

## Execution

- **Startup**: Dispatcher launches on deploy
- **Auto-restart**: Dispatcher restarts on failure/heartbeat timeout

## Behavior

- Source: only one instance runs at a time (activation: source)
- Emits heartbeat every 30s
- Handles RPC disconnects with automatic reconnection
- Emits upstream event to Dispatcher when new blocks written

### Tip Continuity

- Tracks the last ingested chain tip (height + hash)
- If head jumps forward, backfills missing blocks by number (`last+1..head`)
- Verifies continuity by checking `parent_hash` between successive blocks

### Reorg Handling (Tip)

- Detects a reorg when the next block’s `parent_hash` does not match the stored tip hash
- Walks backwards by height (via RPC) until it finds a common ancestor where `rpc.hash == stored.hash`
- Deletes orphaned blocks from hot storage, then ingests the canonical branch forward
- Records a `data_invalidations` row-range invalidation for downstream reprocessing (see [data_versioning.md](../data_versioning.md#reorg-handling))

### Deep Reorg (Before start_block)

If the reorg walks back to or before `start_block`:
1. All hot data is invalidated
2. Truncate hot storage for this dataset
3. Re-ingest from `start_block` forward
4. Emit invalidation covering full range so downstream jobs rebuild

### Cold Start / Initial Sync

On first run (empty hot storage), `block_follower` requires a starting point:

```yaml
config:
  start_block: 1000000   # Required: block to start from
```

If `start_block` is not set, operator fails with a configuration error.

## Dependencies

- RPC provider access (WebSocket preferred)
- Postgres write access to hot tables

## Example DAG Config

```yaml
- name: block_follower
  activation: source
  runtime: ecs_rust
  operator: block_follower
  source:
    kind: always_on
  config:
    chain_id: 10143
    rpc_pool: monad
    start_block: 1000000
    emit_strategy: per_update  # emit downstream event per block
  input_datasets: []
  output_datasets: [hot_blocks, hot_logs]
  update_strategy: replace
  heartbeat_timeout_seconds: 60
```
