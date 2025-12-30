# range_splitter

Split a range manifest into per-unit events (inverse of `range_aggregator`).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_rust` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerPartition |
| **Image** | `range_splitter:latest` |

## Description

Consumes a partitioned range manifest event (e.g., `partition_key: "1000000-1010000"`) and emits a stream of finer-grained events (e.g., per-block or per-subrange). This is useful when you need parallelism/fan-out downstream while keeping upstream aggregation explicit.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `partition_key` | event | Range to split (e.g., `"1000000-1010000"`) |
| `chunk_size` | config | Optional subdivision size (e.g., `1000` blocks per emitted event) |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Split events | Dispatcher | Partitioned events (`partition_key`) |

## Behavior

- Parses the incoming `partition_key` range.
- Emits one event per subrange (deterministically) based on `chunk_size`.
- Idempotent under retries (deterministic subdivision).

## Example DAG Config

```yaml
- name: block_range_split
  activation: reactive
  runtime: ecs_rust
  operator: range_splitter
  execution_strategy: PerPartition
  inputs:
    - from: { job: block_range_aggregate, output: 0 }
  outputs: 1
  config:
    chunk_size: 1000
  update_strategy: replace
  timeout_seconds: 60
```

