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
| `alert_id` | partition | Alert definition to evaluate |
| `data_partition` | partition | Data partition to check |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Triggered alerts | `postgres://triggered_alerts` | Rows |
| Evaluation log | `postgres://alert_evaluations` | Rows |

## Execution

- **Data-driven**: New data arrives in watched dataset
- **Cron**: Periodic evaluation of time-based alerts
- **Manual**: Re-evaluate historical data

## Behavior

- Fetches alert definition (condition, thresholds)
- Loads relevant data partition via Polars (zero-copy where possible)
- Evaluates condition using compiled logic
- If triggered: writes to triggered_alerts for delivery
- Logs evaluation result (pass/fail, metrics)

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
- Postgres write access to triggered_alerts

## Example DAG Config

```yaml
- name: alert_evaluate_rs
  activation: reactive
  runtime: ecs_rust
  operator: alert_evaluate_rs
  execution_strategy: PerUpdate
  idle_timeout: 5m
  config: {}
  input_datasets: [hot_blocks, alert_definitions]
  output_dataset: triggered_alerts
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
