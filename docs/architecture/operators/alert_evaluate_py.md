# alert_evaluate_py

Evaluate alert conditions against data (Python implementation).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_udf` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate |
| **Idle Timeout** | `5m` |
| **Image** | `alert_evaluate_py:latest` |

## Description

Evaluates user-defined alert conditions against incoming or historical data. Python implementation — good for ML models, pandas/numpy analysis, statistical detections.

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
- Loads relevant data partition (via pandas/polars)
- Evaluates condition using user-defined logic
- If triggered: writes alert record(s) to an object-store scratch batch artifact and requests publish to the `alert_events` buffered sink via `POST /internal/buffer-publish` (deduped downstream by `dedupe_key` )

## Condition Types Supported

| Type | Description | Example |
|------|-------------|---------|
| Statistical | pandas/numpy analysis | `df['value'].std() > threshold` |
| ML Model | scikit-learn, pytorch | `model.predict(features) > 0.9` |
| Time Series | Anomaly detection | `prophet.detect_anomaly(ts)` |
| Custom | User-provided function | Python expression |

## Dependencies

- Query Service read access to `alert_definitions` (scoped by org)
- Scoped object storage access for reading inputs and writing scratch artifacts
- Wrapper-mediated `POST /internal/buffer-publish` (no direct queue permissions)
- Python packages: pandas, numpy, scikit-learn (as needed)

## Example DAG Config

```yaml
- name: alert_evaluate_py
  activation: reactive
  runtime: ecs_udf
  operator: alert_evaluate_py
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
