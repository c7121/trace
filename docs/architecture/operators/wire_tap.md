# wire_tap

Copy events to a secondary destination without affecting the main flow.

See [Wire Tap pattern](https://www.enterpriseintegrationpatterns.com/patterns/messaging/WireTap.html) (Enterprise Integration Patterns).

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `dispatcher` |
| **Activation** | `reactive` |
| **Execution** | Dispatcher (no worker) |

## Description

Virtual operator handled by the Dispatcher — no worker spins up. Intercepts events flowing through the DAG and copies them to a secondary destination for debugging, auditing, or analysis.

## What Can Be Tapped

Given our event-based system, wire taps operate on **events**, not raw data:

| Tap Target | Description | Destination Options |
|------------|-------------|---------------------|
| Event stream | `{dataset, cursor}` events | CloudWatch Logs, S3 (JSON), SNS |
| Job metadata | Execution timing, status | CloudWatch Metrics, S3 |

**Note:** Wire tap does NOT copy Parquet data. For dataset snapshots/backup, use a real operator that reads and writes files.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `input_datasets` | config | Datasets to tap |
| `tap_destination` | config | Where to send copies (cloudwatch, s3, sns) |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Original event | Downstream jobs | Unchanged, normal routing |
| Tapped copy | `tap_destination` | Event JSON |

## Behavior

1. Dispatcher receives event for tapped dataset
2. Copies event to `tap_destination` (async, fire-and-forget)
3. Routes original event to downstream jobs normally
4. Tap failure does NOT block main flow

## Example DAG Config

```yaml
# Tap all block events for debugging
- name: debug_blocks_tap
  activation: reactive
  runtime: dispatcher
  operator: wire_tap
  input_datasets: [hot_blocks]
  tap_destination: cloudwatch  # or s3://bucket/taps/

# Tap alert events to SNS for external monitoring
- name: alert_audit_tap
  activation: reactive
  runtime: dispatcher
  operator: wire_tap
  input_datasets: [alert_results]
  tap_destination: sns://alert-audit-topic
```

## Use Cases

1. **Debugging** — Log all events from a specific dataset during development
2. **Auditing** — Record alert firings for compliance
3. **Monitoring** — Feed events to external systems without modifying the DAG
4. **Replay** — Capture events to S3 for later replay/testing
