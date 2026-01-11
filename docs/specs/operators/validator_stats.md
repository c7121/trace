# validator_stats

Track validator performance and emit per-epoch stats.

Status: planned

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `lambda` |
| **Activation** | `source` |
| **Source Kind** | `cron` |
| **Image** | `validator_stats:latest` |

## Description

Periodically fetches validator duties/attestations/rewards from chain APIs and writes normalized performance rows to `validator_performance` for alerting and dashboards.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `chain_id` | config | Target chain |
| `rpc_pool` | config | RPC provider pool to use |
| `validators` | config | Which validators to track (ids/pubkeys) |
| `epoch_lookback` | config | How many epochs to recompute each run |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Validator performance | `postgres://validator_performance` | Rows |

## Execution

- **Cron**: Runs on a fixed schedule (e.g., every 5 minutes)

## Behavior

- Determines current epoch (per chain)
- Fetches validator stats for epoch range (current + lookback)
- Writes performance rows keyed by validator + epoch
- Emits an upstream event so downstream jobs/alerts can react

## Dependencies

- RPC / beacon API access via the RPC Egress Gateway (or in-VPC nodes)
- Postgres write access to `validator_performance`

## Example DAG config

```yaml
- name: validator_stats
  activation: source
  runtime: lambda
  operator: validator_stats
  source:
    kind: cron
    schedule: "*/5 * * * *"
  secrets: [monad_rpc_key]
  config:
    chain_id: 10143
    rpc_pool: monad
    validators: [0xabc..., 0xdef...]
    epoch_lookback: 2
  outputs: 1
  update_strategy: replace
  timeout_seconds: 60
```

## Related

- Recipe: [validator_monitoring.md](../../examples/validator_monitoring.md)
