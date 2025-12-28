# alert_evaluate_ts

Evaluate alert conditions against data (TypeScript implementation).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | TypeScript |
| **Execution Strategy** | PerPartition |
| **Image** | `alert_evaluate_ts:latest` |

## Description

Evaluates user-defined alert conditions against incoming or historical data. TypeScript implementation — good for JSON-heavy conditions, complex logic, async data fetches.

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

## Triggers

- **Data-driven**: New data arrives in watched dataset
- **Cron**: Periodic evaluation of time-based alerts
- **Manual**: Re-evaluate historical data

## Behavior

- Fetches alert definition (condition, thresholds)
- Loads relevant data partition
- Evaluates condition using user-defined logic
- If triggered: writes to triggered_alerts for delivery
- Logs evaluation result (pass/fail, metrics)

## Condition Types Supported

| Type | Description | Example |
|------|-------------|---------|
| Threshold | Value exceeds limit | `balance > 1000 ETH` |
| Pattern | Matches address/signature | `to_address IN [...]` |
| Anomaly | Deviation from baseline | `tx_count > 3σ` |
| Custom | User-provided function | JS/TS expression |

## Dependencies

- Postgres read access to alert_definitions
- Data storage read access (S3/Postgres)
- Postgres write access to triggered_alerts

## Example DAG Config

```yaml
- name: alert_evaluate_ts
  job_type: Transform
  execution_strategy: PerPartition
  runtime: TypeScript
  entrypoint: alert_evaluate/index.ts
  config: {}
  input_datasets: [hot_blocks, alert_definitions]
  output_dataset: triggered_alerts
  timeout_seconds: 120
```
