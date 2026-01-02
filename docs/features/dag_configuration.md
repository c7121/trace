# DAG Configuration

How DAGs are defined in YAML and deployed to the system.

Users create and edit DAG configurations via the API or UI. Each DAG is stored and versioned by the system.

## Concepts

- **Operator** — the implementation (code) that runs in a runtime (`lambda`, `ecs_platform`, `ecs_udf`). **Trust is determined by the operator** (platform-managed vs user/UDF bundle), not by the compute primitive. Treat `lambda` as untrusted unless the operator is platform-managed.
- **Job** — a configured instance of an operator inside a DAG (`runtime` + `operator` + `config`).
- **Dataset** — a user-facing, **published** output with:
  - `dataset_name` (human-readable string, unique per org) and
  - `dataset_uuid` (system UUID primary key used internally + in storage paths).
  The registry is the authoritative mapping from name → UUID and dataset metadata (see [ADR 0008](../architecture/adr/0008-dataset-registry-and-publishing.md)); versioned storage locations are resolved via `dataset_versions` + DAG pointer sets (see [ADR 0009](../architecture/adr/0009-atomic-cutover-and-query-pinning.md)). Some datasets (e.g., buffered sink datasets like `alert_events`) are multi-writer within a DAG. Reads are gated by registry ACLs (`datasets.read_roles`) for Query Service and cross-DAG `inputs: from: { dataset: ... }`.
- **Edge** — an internal connection from an upstream job output **by index** (e.g., `block_follower.output[0]`) to a downstream job input. Internal edges do **not** require dataset naming.
- **Publish** — a top-level mapping that registers a specific `{job, output_index}` as a user-visible dataset in the registry (metadata-only; does not change execution/backfill).
- **Filter** — an optional read-time predicate on an input edge (e.g., consume only `severity = 'critical'`). Filters are applied by the consumer, not the Dispatcher (see ADR 0007).
- **Trigger** — what causes a job to run:
  - For `activation: source` jobs, the trigger is `source.kind` (`cron`, `webhook`, `manual`, or `always_on`).
  - For `activation: reactive` jobs, the trigger is an upstream **output event** on any `inputs` edge (1 upstream event → 1 task; no dispatcher-side bulk/coalescing).
- **Bootstrap** — optional one-time initialization for `activation: source` jobs at activation/deploy (v1: `reset_outputs`).
- **Ordering** — the `jobs:` list order is not significant; dependencies are resolved by explicit `inputs` edges.
- **Multi-source** — model multiple input streams as multiple source jobs. Each source stream has its own ordered cursor; aggregate per source as needed (e.g., `range_aggregator`), then join downstream.
- **Concurrency** — optional scaling hint (`scaling.max_concurrency`). Provider key pooling and rate limiting are handled by the RPC Egress Gateway (not by per-worker secret slots in YAML).

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
    outputs: 1
    update_strategy: replace

  # Source: always-running block follower
  - name: block_follower
    activation: source
    runtime: ecs_platform
    operator: block_follower
    source:
      kind: always_on
    # Optional: wipe job-owned outputs and restart from start_block (executed once at activation/deploy)
    # bootstrap:
    #   reset_outputs: true
    config:
      chain_id: 10143
      rpc_pool: monad
      start_block: 1000000
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
    runtime: ecs_udf
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
    runtime: ecs_platform
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
    runtime: ecs_platform
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
    runtime: ecs_platform
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

  cold_blocks:
    from: { job: compact_blocks, output: 0 }
    storage: s3
```

## Job Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | ✅ | Unique job name within DAG |
| `activation` | ✅ | `source` or `reactive` |
| `runtime` | ✅ | `lambda`, `ecs_platform`, `ecs_udf`, `dispatcher` |
| `operator` | ✅ | Operator implementation to run |
| `outputs` | ✅ | Number of outputs exposed as `output[0..N-1]` for wiring and publishing |
| `update_strategy` | ✅ | `append` or `replace` — how outputs are written |
| `unique_key` | if append | Required for `append` — columns for dedupe |
| `inputs` | reactive | Upstream edges (`from: {job, output}` or `from: {dataset: dataset_name}`), optionally with `where` |
| `execution_strategy` | reactive | `PerUpdate` or `PerPartition` (Bulk is not supported; use explicit Aggregator/Splitter patterns) |
| `source` | source | Source config: `kind`, `schedule`, etc. |
| `bootstrap` | source | Optional one-time bootstrap actions for sources (v1: `reset_outputs`) |
| `config` | | Operator-specific config |
| `secrets` | | Secret names to inject as env vars |
| `scaling` | | Optional scaling hints (v1: `max_concurrency`) |
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
| `write_mode` | postgres | `buffered` (queue → sink → table) or `direct` (platform jobs only) |
| `schema` | buffered | Table schema: `columns`, `unique`, `indexes` |

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

### Concurrency and RPC credentials

For jobs that need high-concurrency RPC access (e.g., Cryo backfills), use `config.rpc_pool` and set `scaling.max_concurrency`. The RPC Egress Gateway owns key pooling, rotation, and rate limiting. Avoid modeling per-task unique credentials as DAG YAML primitives in v1.

### Update Strategy & Unique Key

Every job must declare `update_strategy`:
- `replace` — overwrites output for the processed scope (partition or cursor range)
- `append` — inserts rows, dedupes by `unique_key`

If `update_strategy: append`, `unique_key` is **required** and must be deterministic (derived from input data only). See [data_versioning.md](../architecture/data_versioning.md#unique-key-requirements) for the full specification.

## Deployment

DAG YAML is parsed, validated, and synced into the `jobs` table. See [dag_deployment.md](../architecture/dag_deployment.md) for the deploy/sync flow and database semantics.
