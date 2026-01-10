# liveliness_monitor

Detect when a chain stops producing blocks and emit liveliness events.

Status: Planned

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `lambda` |
| **Activation** | `source` |
| **Source Kind** | `cron` |
| **Image** | `liveliness_monitor:latest` |

## Description

Periodically polls a chain tip endpoint, computes the time since the last block, and writes `liveliness_events`. Intended for quick detection of stalled chains and degraded block production.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `chain_id` | config | Target chain |
| `rpc_pool` | config | RPC provider pool to use |
| `expected_block_time_ms` | config | Expected block interval for this chain |
| `alert_threshold_ms` | config | Gap threshold before emitting an alert-level event |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Liveliness events | `postgres://liveliness_events` | Rows |

## Execution

- **Cron**: Runs on a fixed schedule (e.g., every minute)

## Behavior

- Fetches the latest block timestamp/number from RPC
- Computes `now - last_block_time`
- Writes an event row with chain id, gap, and status (ok/degraded/stalled)
- Emits an upstream event so downstream jobs/alerts can react

## Dependencies

- RPC provider access
- Postgres write access to `liveliness_events`

## Example DAG Config

```yaml
- name: chain_liveliness
  activation: source
  runtime: lambda
  operator: liveliness_monitor
  source:
    kind: cron
    schedule: "*/1 * * * *"
  secrets: [monad_rpc_key]
  config:
    chain_id: 10143
    rpc_pool: monad
    expected_block_time_ms: 500
    alert_threshold_ms: 5000
  outputs: 1
  update_strategy: replace
  timeout_seconds: 60
```

Recipe: [Chain liveliness monitoring](../../examples/chain_liveliness_monitoring.md)
