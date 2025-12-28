# block_follower

Follow chain tip in real-time, writing new blocks to hot storage.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | Rust |
| **Execution Strategy** | Singleton |
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

## Triggers

- **Startup**: Dispatcher launches on deploy
- **Auto-restart**: Dispatcher restarts on failure/heartbeat timeout

## Behavior

- Singleton: only one instance runs at a time
- Emits heartbeat every 30s
- Handles RPC disconnects with automatic reconnection
- Emits threshold event to Trigger Service when block count reached

### Reorg Handling (Real-time)

- Maintains local chain of recent block hashes (in memory)
- On new block: checks parent hash against local tip
- If mismatch (reorg detected):
  - Identifies fork point (common ancestor)
  - Deletes/rolls back orphaned blocks from hot storage
  - Re-indexes canonical chain from fork point
- Hot storage is mutable — reorgs are handled immediately

## Dependencies

- RPC provider access (WebSocket preferred)
- Postgres write access to hot tables

## Example DAG Config

```yaml
- name: block_follower
  job_type: Ingest
  execution_strategy: Singleton
  runtime: Rust
  entrypoint: block_follower
  config:
    chain_id: 10143
    rpc_pool: monad
    threshold_blocks: 10000
  input_datasets: []
  output_dataset: hot_blocks
  heartbeat_timeout_seconds: 60
```
