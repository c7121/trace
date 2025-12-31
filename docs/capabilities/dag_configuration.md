# DAG Configuration

How DAGs are defined in YAML and deployed to the system.

Users create and edit DAG configurations via the API or UI. Each DAG is stored and versioned by the system.

## Concepts

- **Operator** — the implementation (code) that runs in a runtime (`lambda`, `ecs_rust`, etc).
- **Job** — a configured instance of an operator inside a DAG (`runtime` + `operator` + `config`).
- **Dataset** — a user-facing, **published** output with:
  - `dataset_name` (human-readable string, unique per org) and
  - `dataset_uuid` (system UUID primary key used internally + in storage paths).
  The registry is the authoritative mapping from name → UUID and dataset metadata (see [ADR 0008](../architecture/adr/0008-dataset-registry-and-publishing.md)); versioned storage locations are resolved via `dataset_versions` + DAG pointer sets (see [ADR 0009](../architecture/adr/0009-atomic-cutover-and-query-pinning.md)). Some datasets (e.g., buffered sink datasets like `alert_events`) are multi-writer within a DAG.
- **Edge** — an internal connection from an upstream job output **by index** (e.g., `block_follower.output[0]`) to a downstream job input. Internal edges do **not** require dataset naming.
- **Publish** — a top-level mapping that registers a specific `{job, output_index}` as a user-visible dataset in the registry (metadata-only; does not change execution/backfill).
- **Filter** — an optional read-time predicate on an input edge (e.g., consume only `severity = 'critical'`). Filters are applied by the consumer, not the Dispatcher (see ADR 0007).
- **Trigger** — what causes a job to run:
  - For `activation: source` jobs, the trigger is `source.kind` (`cron`, `webhook`, `manual`, or `always_on`).
  - For `activation: reactive` jobs, the trigger is an upstream **output event** on any `inputs` edge (1 upstream event → 1 task; no dispatcher-side bulk/coalescing).
- **Ordering** — the `jobs:` list order is not significant; dependencies are resolved by explicit `inputs` edges.
- **Worker pool** — optional named pool of per-worker “slots” (env + secrets) used when concurrent tasks must run with distinct credentials (e.g., Cryo backfills where each task needs a unique RPC key). Jobs reference a pool and set `scaling.max_concurrency`; effective concurrency is limited by pool size.

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

worker_pools:
  # Each worker pool is an explicit list of “slots”. The Dispatcher leases a slot per running task.
  # `secret_env` maps env var name -> secret name (so all slots can expose the same env var names,
  # but resolve to different underlying secrets).
  monad_rpc_keys:
    slots:
      - secret_env:
          MONAD_RPC_KEY: monad_rpc_key_1
      - secret_env:
          MONAD_RPC_KEY: monad_rpc_key_2

jobs:
  # Source: Lambda cron emits daily event
  - name: daily_trigger
    activation: source
    runtime: lambda
    operator: cron_source
    source:
      kind: cron
      schedule: "0 0 * * *"
    outputs: 1
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
    outputs: 2
    update_strategy: replace
    heartbeat_timeout_seconds: 60

  # Source: manual backfill requests
  - name: backfill_request
    activation: source
    runtime: lambda
    operator: manual_source
    source:
      kind: manual
    outputs: 1
    update_strategy: replace
    
  # Reactive: evaluate alerts on new blocks
  - name: alert_evaluate_rs
    activation: reactive
    runtime: lambda
    operator: alert_evaluate_rs
    execution_strategy: PerUpdate # cursor-based event input
    idle_timeout: 5m
    inputs:
      - from: { job: block_follower, output: 0 }
    outputs: 1
    update_strategy: append
    unique_key: [dedupe_key]
    timeout_seconds: 60
    
  # Record-count batching: aggregate ordered updates into deterministic ranges (EIP Aggregator)
  - name: block_range_aggregate
    activation: reactive
    runtime: ecs_rust
    operator: range_aggregator
    execution_strategy: PerUpdate
    inputs:
      - from: { job: block_follower, output: 0 }
    config:
      cursor_column: block_number
      range_size: 10000
    outputs: 1 # emits range manifests (partition_key like "1000000-1010000")
    update_strategy: append
    unique_key: [dedupe_key]
    timeout_seconds: 60

  # Compaction: consume range manifests and write cold Parquet partitions
  - name: compact_blocks
    activation: reactive
    runtime: ecs_rust
    operator: parquet_compact
    execution_strategy: PerPartition
    inputs:
      - from: { job: block_range_aggregate, output: 0 }
    outputs: 1
    update_strategy: replace
    timeout_seconds: 1800
    
  # Backfill: manual source emits partitioned backfill requests
  - name: cryo_backfill
    activation: reactive
    runtime: ecs_rust
    operator: cryo_ingest
    execution_strategy: PerPartition
    idle_timeout: 0
    inputs:
      - from: { job: backfill_request, output: 0 }
  config:
    chain_id: 10143
    datasets: [blocks, transactions, logs]
    rpc_pool: monad
  scaling:
    worker_pool: monad_rpc_keys
    max_concurrency: 20
    outputs: 3
    update_strategy: replace
    timeout_seconds: 3600

publish:
  # Publish is metadata-only: it registers a `{job, output_index}` as a user-visible dataset in the registry.
  # Publishing does not change how the DAG runs; it only affects discoverability + Query Service exposure.

  hot_blocks:
    from: { job: block_follower, output: 0 }

  hot_logs:
    from: { job: block_follower, output: 1 }

  alert_events:
    from: { job: alert_evaluate_rs, output: 0 }
    storage: postgres
    write_mode: buffered
    schema:
      columns:
        - { name: org_id, type: uuid, nullable: false }
        - { name: dedupe_key, type: text, nullable: false }
        - { name: severity, type: text, nullable: true }
        - { name: payload, type: jsonb, nullable: false }
        - { name: event_time, type: timestamptz, nullable: false }
        - { name: created_at, type: timestamptz, nullable: false }
      unique: [org_id, dedupe_key]
      indexes:
        - [org_id, event_time]
    buffer:
      kind: sqs
      fifo: true

  cold_blocks:
    from: { job: compact_blocks, output: 0 }
    storage: s3
```

## Job Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | ✅ | Unique job name within DAG |
| `activation` | ✅ | `source` or `reactive` |
| `runtime` | ✅ | `lambda` (TypeScript/JavaScript, Python, or Rust), `ecs_rust`, `ecs_python`, `dispatcher` |
| `operator` | ✅ | Operator implementation to run |
| `outputs` | ✅ | Number of outputs exposed as `output[0..N-1]` for wiring and publishing |
| `update_strategy` | ✅ | `append` or `replace` — how outputs are written |
| `unique_key` | if append | Required for `append` — columns for dedupe |
| `inputs` | reactive | Upstream edges (`from: {job, output}` or `from: {dataset: dataset_name}`), optionally with `where` |
| `execution_strategy` | reactive | `PerUpdate` or `PerPartition` (Bulk is not supported; use explicit Aggregator/Splitter patterns) |
| `source` | source | Source config: `kind`, `schedule`, etc. |
| `config` | | Operator-specific config |
| `secrets` | | Secret names to inject as env vars |
| `scaling` | | Optional scaling hints (v1: `worker_pool`, `max_concurrency`) |
| `timeout_seconds` | | Max execution time |

### Input Filters

`inputs` supports a long form for read-time filtering:

```yaml
inputs:
  - from: { dataset: alert_events }
    where: "severity = 'critical' AND chain_id = 1"
```

The Dispatcher routes by the upstream output identity only; the consumer applies `where` when reading. See [ADR 0007](../architecture/adr/0007-input-edge-filters.md) for the v1 predicate rules.

## Publish Fields (Optional)

Published datasets can optionally include storage configuration (and for buffered Postgres datasets, schema + buffering). If omitted, storage/backing is resolved via the registry and/or the producing operator’s defaults. See [ADR 0006](../architecture/adr/0006-buffered-postgres-datasets.md).

| Field | Required | Description |
|-------|----------|-------------|
| `from` | ✅ | `{job, output}` reference to publish |
| `storage` | | `postgres` or `s3` (optional if preconfigured/implicit) |
| `write_mode` | postgres | `buffered` (SQS → sink → table) or `direct` (platform jobs only) |
| `schema` | buffered | Table schema: `columns`, `unique`, `indexes` |
| `buffer` | buffered | Buffer config (v1: `kind: sqs`, optional `fifo`) |

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

See [security_model.md](../standards/security_model.md#secrets-injection) for how secrets are injected.

### Worker Pools (Per-Worker Secrets)

Some jobs need **distinct credentials per concurrent task** (e.g., Cryo backfills where each task must use a different RPC key). In that case:

1. Define a `worker_pools` entry with explicit `slots`.
2. Configure the job’s `scaling.worker_pool` and `scaling.max_concurrency`.

Each slot can provide:
- `env`: plain env vars injected into the worker container.
- `secret_env`: mapping of env var name → secret name. This keeps the env var **stable** (e.g., always `MONAD_RPC_KEY`) while allowing each slot to resolve a different secret.

`scaling.mode` is ignored/unsupported in v1.

### Update Strategy & Unique Key

Every job must declare `update_strategy`:
- `replace` — overwrites output for the processed scope (partition or cursor range)
- `append` — inserts rows, dedupes by `unique_key`

If `update_strategy: append`, `unique_key` is **required** and must be deterministic (derived from input data only). See [data_versioning.md](../architecture/data_versioning.md#unique-key-requirements) for the full specification.

## Deployment

DAG YAML is parsed, validated, and synced into the `jobs` table. See [dag_deployment.md](../architecture/dag_deployment.md) for the deploy/sync flow and database semantics.
