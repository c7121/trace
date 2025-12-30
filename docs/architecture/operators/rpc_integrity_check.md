# rpc_integrity_check

Cross-check RPC providers and emit divergence events.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `lambda` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate |
| **Idle Timeout** | `5m` |
| **Image** | `rpc_integrity_check:latest` |

## Description

For each new block event, fetch the same block from a configured set of RPC providers and compare key fields (hash, state root, tx count, etc). Writes `rpc_divergence_events` rows when providers disagree.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `chain_id` | config | Target chain |
| `rpc_pool` | config | RPC provider pool to sample from |
| `checks` | config | Fields to compare (e.g., `block_hash`, `state_root`) |
| `min_agreement` | config | Quorum needed to consider data valid |
| `block_number` | input (from `hot_blocks`) | Block to validate |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Divergence events | `postgres://rpc_divergence_events` | Rows |

## Execution

- **Reactive**: Runs per `hot_blocks` update (`PerUpdate`)
- **Optional cron**: Can be configured as a `source` cron job to sample latest block periodically

## Behavior

- For the target block, queries N providers in parallel
- Compares configured fields across responses
- Writes divergence event with provider set, differing values, and metadata for triage
- Emits an upstream event so downstream jobs/alerts can react

## Dependencies

- RPC provider access (allowlisted endpoints)
- Postgres write access to `rpc_divergence_events`

## Example DAG Config

```yaml
- name: rpc_integrity_check
  activation: reactive
  runtime: lambda
  operator: rpc_integrity_check
  execution_strategy: PerUpdate
  idle_timeout: 5m
  inputs:
    - from: { job: block_follower, output: 0 }
  secrets: [monad_rpc_key]
  config:
    chain_id: 10143
    rpc_pool: monad
    checks: [block_hash, state_root, tx_count]
    min_agreement: 2
  outputs: 1
  update_strategy: replace
  timeout_seconds: 60
```
