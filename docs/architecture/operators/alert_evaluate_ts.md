# alert_evaluate_ts

Evaluate alert conditions against data (TypeScript implementation).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `lambda` (v1); `ecs_udf` deferred to v2 |
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
- Loads relevant data partition
- Evaluates condition using user-defined logic
- If triggered: writes alert record(s) to an object-store scratch batch artifact and requests publish to the `alert_events` buffered sink via `POST /v1/task/buffer-publish` (deduped downstream by `dedupe_key`)

## Condition Types Supported

| Type | Description | Example |
|------|-------------|---------|
| Threshold | Value exceeds limit | `balance > 1000 ETH` |
| Pattern | Matches address/signature | `to_address IN [...]` |
| Anomaly | Deviation from baseline | `tx_count > 3σ` |
| Custom | User-provided function | JS/TS expression |

## Dependencies

- Query Service read access to `alert_definitions` (scoped by org)
- Scoped object storage access for reading inputs and writing scratch artifacts
- Wrapper-mediated `POST /v1/task/buffer-publish` (no direct queue permissions)

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
