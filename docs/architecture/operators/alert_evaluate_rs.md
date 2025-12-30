# alert_evaluate_rs

Evaluate alert conditions against data (Rust/Polars implementation).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_rust` |
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
| `alert_definitions` | storage | Postgres table of enabled alert definitions (read) |

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
- If triggered: publishes an alert record to the `alert_events` dataset buffer (deduped downstream by `dedupe_key`)

## Condition Types Supported

| Type | Description | Example |
|------|-------------|---------|
| Threshold | Fast numeric comparison | `col("value").gt(threshold)` |
| Filter | Address/signature matching | `col("to").is_in(watch_list)` |
| Aggregate | Window functions | `col("amount").rolling_sum(100)` |
| Expression | Polars expression DSL | User-defined Polars expr |

## Dependencies

- Postgres read access to alert_definitions
- Data storage read access (S3/Postgres)
- SQS send access to the `alert_events` dataset buffer

## Example DAG Config

```yaml
- name: alert_evaluate_rs
  activation: reactive
  runtime: ecs_rust
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
