# alert_evaluate_ts

Evaluate alert conditions against data (TypeScript implementation).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `lambda` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate |
| **Idle Timeout** | `5m` |
| **Image** | `alert_evaluate_ts:latest` |

## Description

Evaluates user-defined alert conditions against incoming or historical data. TypeScript implementation — good for JSON-heavy conditions and complex logic.

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
- Loads relevant data partition
- Evaluates condition using user-defined logic
- If triggered: publishes an alert record to the `alert_events` dataset buffer (deduped downstream by `dedupe_key`)

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
- SQS send access to the `alert_events` dataset buffer

## Example DAG Config

```yaml
- name: alert_evaluate_ts
  activation: reactive
  runtime: lambda
  operator: alert_evaluate_ts
  execution_strategy: PerUpdate
  idle_timeout: 5m
  config: {}
  inputs:
    - from: { job: block_follower, output: 0 }
  outputs: 1
  update_strategy: append
  unique_key: [dedupe_key]
  timeout_seconds: 120
```
