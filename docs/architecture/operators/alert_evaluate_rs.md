# alert_evaluate_rs

Evaluate alert conditions against data (Rust/Polars implementation).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_udf` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate |
| **Idle Timeout** | `5m` |
| **Image** | `alert_evaluate_rs:latest` |

## Description

Evaluates user-defined alert conditions against incoming or historical data. Rust implementation with Polars — best for high-performance scanning, large datasets, low-latency alerting.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `cursor` | event | Cursor value from upstream dataset event (e.g., `block_number`) |
| `alert_definitions` | storage | Query Service view/table of enabled alert definitions (scoped read) |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Alert events | `postgres://alert_events` (buffered sink) | Rows |

## Execution

- **Data-driven**: New data arrives in watched dataset
- **Cron**: Periodic evaluation of time-based alerts
- **Manual**: Re-evaluate historical data

## Behavior

- Fetches alert definition (condition, thresholds)
- Loads relevant data partition via Polars (zero-copy where possible)
- Evaluates condition using compiled logic
- If triggered: writes alert record(s) to an object-store scratch batch artifact and requests publish to the `alert_events` buffered sink via `POST /v1/task/buffer-publish` (deduped downstream by `dedupe_key`)

## Condition Types Supported

| Type | Description | Example |
|------|-------------|---------|
| Threshold | Fast numeric comparison | `col("value").gt(threshold)` |
| Filter | Address/signature matching | `col("to").is_in(watch_list)` |
| Aggregate | Window functions | `col("amount").rolling_sum(100)` |
| Expression | Polars expression DSL | User-defined Polars expr |

## Dependencies

- Query Service read access to `alert_definitions` (scoped by org)
- Scoped object storage access for reading inputs and writing scratch artifacts
- Wrapper-mediated `POST /v1/task/buffer-publish` (no direct queue permissions)

## Example DAG Config

```yaml
- name: alert_evaluate_rs
  activation: reactive
  runtime: ecs_udf
  operator: alert_evaluate_rs
  execution_strategy: PerUpdate
  idle_timeout: 5m
  config: {}
  inputs:
    - from: { job: block_follower, output: 0 }
  outputs: 1
  update_strategy: append
  unique_key: [dedupe_key]
  timeout_seconds: 60
```


## When to Use

Choose Rust/Polars when:
- Scanning large datasets (millions of rows)
- Low-latency alerting required
- Simple, well-defined conditions
- Memory efficiency matters

Choose Python or TypeScript when:
- Complex ML models
- Rapid prototyping
- Rich ecosystem needed (pandas, scikit-learn)
