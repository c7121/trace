# range_aggregator

Aggregate an ordered stream of events into deterministic range manifests (EIP Aggregator).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_rust` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate |
| **Image** | `range_aggregator:latest` |

## Description

Consumes an ordered stream of upstream events (e.g., `block_number` updates) and emits **range manifests** as partitioned events with `partition_key` like `"1000000-1010000"` (inclusive).

This makes “bulk/compaction” behavior explicit in the DAG (instead of implicit Dispatcher coalescing).

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `cursor` | event | Current cursor value (e.g., `block_number`) |
| `cursor_column` | config | Name of the ordering column (informational) |
| `range_size` | config | Size of each emitted range (e.g., `10000` blocks) |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Range manifests | Dispatcher | Partitioned events (`partition_key`) |

## Behavior

- Maintains durable aggregation state per input stream (e.g., `{last_emitted_end, pending_start}`).
- On each incoming cursor event:
  - Advances internal cursor tracking
  - When enough progress is observed to form one or more full ranges, emits one manifest event per range.
- Emission is idempotent under retries (deterministic ranges + unique/constraint-backed bookkeeping).

## Example DAG Config

```yaml
- name: block_range_aggregate
  activation: reactive
  runtime: ecs_rust
  operator: range_aggregator
  execution_strategy: PerUpdate
  inputs:
    - from: { job: block_follower, output: 0 }
  outputs: 1
  config:
    cursor_column: block_number
    range_size: 10000
  update_strategy: append
  unique_key: [dedupe_key]
  timeout_seconds: 60
```

