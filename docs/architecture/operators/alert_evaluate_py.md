# alert_evaluate_py

Evaluate alert conditions against data (Python implementation).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | Python |
| **Execution Strategy** | PerPartition |
| **Image** | `alert_evaluate_py:latest` |

## Description

Evaluates user-defined alert conditions against incoming or historical data. Python implementation — good for ML models, pandas/numpy analysis, statistical detections.

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
- Loads relevant data partition (via pandas/polars)
- Evaluates condition using user-defined logic
- If triggered: writes to triggered_alerts for delivery
- Logs evaluation result (pass/fail, metrics)

## Condition Types Supported

| Type | Description | Example |
|------|-------------|---------|
| Statistical | pandas/numpy analysis | `df['value'].std() > threshold` |
| ML Model | scikit-learn, pytorch | `model.predict(features) > 0.9` |
| Time Series | Anomaly detection | `prophet.detect_anomaly(ts)` |
| Custom | User-provided function | Python expression |

## Dependencies

- Postgres read access to alert_definitions
- Data storage read access (S3/Postgres)
- Postgres write access to triggered_alerts
- Python packages: pandas, numpy, scikit-learn (as needed)

## Example DAG Config

```yaml
- name: alert_evaluate_py
  job_type: Transform
  execution_strategy: PerPartition
  runtime: Python
  entrypoint: alert_evaluate.main:evaluate
  config: {}
  input_datasets: [hot_blocks, alert_definitions]
  output_dataset: triggered_alerts
  timeout_seconds: 120
```
