# DAG Configuration

How DAGs are defined in YAML and deployed to the system.

Users create and edit DAG configurations via the API or UI. Each DAG is stored and versioned by the system.

## Concepts

- **Operator** — the implementation (code) that runs in a runtime (`lambda`, `ecs_rust`, etc).
- **Job** — a configured instance of an operator inside a DAG (`runtime` + `operator` + `config`).
- **Trigger** — what causes a job to run:
  - For `activation: source` jobs, the trigger is `source.kind` (`cron`, `webhook`, `manual`, or `always_on`).
  - For `activation: reactive` jobs, the trigger is an upstream dataset event on any `input_datasets` (fan-out shaped by `execution_strategy`).
- **Ordering** — the `jobs:` list order is not significant; dependencies are resolved by dataset names (`input_datasets` / `output_datasets`).

## YAML Schema

```yaml
name: cross-chain-analytics

defaults:
  heartbeat_timeout_seconds: 60
  max_attempts: 3
  priority: normal
  max_queue_depth: 1000
  max_queue_age: 5m
  backpressure_mode: pause

jobs:
  # Source: Lambda cron emits daily event
  - name: daily_trigger
    activation: source
    runtime: lambda
    operator: cron_source
    source:
      kind: cron
      schedule: "0 0 * * *"
    output_datasets: [daily_events]
    update_strategy: replace

  # Source: always-running block follower
  - name: block_follower
    activation: source
    runtime: ecs_rust
    operator: block_follower
    source:
      kind: always_on
    config:
      chain_id: 10143
      rpc_pool: monad
    output_datasets: [hot_blocks, hot_logs]
    update_strategy: replace
    heartbeat_timeout_seconds: 60

  # Source: manual backfill requests
  - name: backfill_request
    activation: source
    runtime: lambda
    operator: manual_source
    source:
      kind: manual
    output_datasets: [backfill_requests]
    update_strategy: replace
    
  # Reactive: evaluate alerts on new blocks
  - name: alert_evaluate_rs
    activation: reactive
    runtime: ecs_rust
    operator: alert_evaluate_rs
    execution_strategy: PerUpdate
    idle_timeout: 5m
    input_datasets: [hot_blocks]
    output_datasets: [alert_events]
    update_strategy: append
    unique_key: [dedupe_key]
    timeout_seconds: 60
    
  # Batch: compact triggered by daily cron
  - name: compact_blocks
    activation: reactive
    runtime: ecs_rust
    operator: parquet_compact
    execution_strategy: Bulk
    idle_timeout: 0
    input_datasets: [hot_blocks, daily_events]
    output_datasets: [cold_blocks]
    update_strategy: replace
    timeout_seconds: 1800
    
  # Backfill: manual source emits partitioned backfill requests
  - name: cryo_backfill
    activation: reactive
    runtime: ecs_rust
    operator: cryo_ingest
    execution_strategy: PerPartition
    idle_timeout: 0
    input_datasets: [backfill_requests]
    config:
      chain_id: 10143
      datasets: [blocks, transactions, logs]
    scaling:
      mode: backfill
      max_concurrency: 20
    output_datasets: [cold_blocks, cold_transactions, cold_logs]
    update_strategy: replace
    timeout_seconds: 3600
```

## Job Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | ✅ | Unique job name within DAG |
| `activation` | ✅ | `source` or `reactive` |
| `runtime` | ✅ | `lambda`, `ecs_rust`, `ecs_python`, `dispatcher` |
| `operator` | ✅ | Operator implementation to run |
| `output_datasets` | ✅ | Datasets this job produces |
| `update_strategy` | ✅ | `append` or `replace` — how outputs are written |
| `unique_key` | if append | Required for `append` — columns for dedupe |
| `input_datasets` | reactive | Datasets this job consumes |
| `execution_strategy` | reactive | `PerUpdate`, `PerPartition`, `Bulk` |
| `source` | source | Source config: `kind`, `schedule`, etc. |
| `config` | | Operator-specific config |
| `secrets` | | Secret names to inject as env vars |
| `timeout_seconds` | | Max execution time |

### Secrets

Jobs that need credentials declare them with `secrets`:

```yaml
jobs:
  - name: block_follower
    operator: block_follower
    secrets: [monad_rpc_key]
    config:
      chain_id: 10143
```

See [security.md](../standards/security.md#secrets-injection) for how secrets are injected.

### Update Strategy & Unique Key

Every job must declare `update_strategy`:
- `replace` — overwrites output for the processed scope (partition or cursor range)
- `append` — inserts rows, dedupes by `unique_key`

If `update_strategy: append`, `unique_key` is **required** and must be deterministic (derived from input data only). See [data_versioning.md](../architecture/data_versioning.md#unique-key-requirements) for the full specification.

## Deployment

DAG YAML is parsed, validated, and synced into the `jobs` table. See [dag_deployment.md](../architecture/dag_deployment.md) for the deploy/sync flow and database semantics.
