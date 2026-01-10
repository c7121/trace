# range_aggregator

Aggregate an ordered stream of events into deterministic range manifests (EIP Aggregator).

Status: Planned

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_platform` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate |
| **Image** | `range_aggregator:latest` |

## Description

Consumes an ordered stream of upstream events (e.g., `block_number` updates) and emits **range manifests** as partitioned events with `partition_key` like `"1000000-1010000"` (inclusive).

This makes “bulk/compaction” behavior explicit in the DAG (instead of implicit Dispatcher coalescing).

This is a normal operator node in v1 (not a “virtual” planner node). The inverse operator is [`range_splitter`](range_splitter.md).

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

- Range/window definition belongs to the DAG/operator config (`cursor_column`, `range_size`), not the Dispatcher.
- v1 assumes ordered inputs for a given cursor (monotonic by `cursor`); if inputs are not ordered, behavior is undefined and should be treated as a configuration error.
- Maintains durable aggregation state per input stream (e.g., `{last_emitted_end, pending_start}`).
- On each incoming cursor event:
  - Advances internal cursor tracking
  - When enough progress is observed to form one or more full ranges, emits one manifest event per range.
- Emission is idempotent under retries (deterministic ranges + unique/constraint-backed bookkeeping).

## Output event shape

Each emitted manifest includes both:

- `partition_key`: `"start-end"` (inclusive), and
- explicit range fields: `start`, `end` (inclusive)

See [task_scoped_endpoints.md](../../architecture/contracts/task_scoped_endpoints.md) for the canonical `/v1/task/events` partitioned event shape.

## Durable state

Aggregation state is persisted in a platform-managed Postgres operator state table keyed by `(org_id, job_id)` (and optionally a `state_key` such as `input_dataset_uuid` if a job needs multiple independent cursors).

See `docs/architecture/data_model/orchestration.md` (`operator_state`) for the schema sketch.

## Example DAG Config

```yaml
- name: block_range_aggregate
  activation: reactive
  runtime: ecs_platform
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
