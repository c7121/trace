# DAG configuration

Status: Draft
Owner: Platform
Last updated: 2026-01-11

## Summary
Trace DAGs are defined as a single YAML document that declares jobs (operators + runtime + config), edges (inputs), and optional dataset publishing. The YAML is versioned; a deployed DAG version is the immutable unit of scheduling and lineage.

## Concepts

- **DAG**: a versioned YAML definition containing jobs + edges.
- **Job**: a named node in the DAG. Jobs produce one or more outputs.
- **Task**: a durable unit of work for a job and a specific input update or partition, created and leased by the Dispatcher. A task may be retried.
- **Attempt**: the retry counter for a task. Only the current attempt may heartbeat or complete.
- **Operator**: the stable "what code runs" surface for a job. Operators may be trusted (platform) or untrusted (user bundle) depending on the runtime and operator.
- **UDF job**: a job that executes user-supplied code (untrusted by default).
- **Activation**:
  - `reactive`: runs when upstream outputs update (per-update: 1 upstream event produces 1 task per dependent reactive job, no dispatcher-side coalescing),
  - `source`: long-running follower/producer,
  - `manual`: reserved for future and deploy validation rejects it in v1.
- **Runtime**: where code executes. v1 supports `lambda`, `ecs_platform`, and `dispatcher`. `ecs_udf` is reserved for v2 and deploy validation rejects it in v1.
- **Publish**: register a `{job, output_index}` as a named dataset in the registry (metadata-only, see [ADR 0008](../adr/0008-dataset-registry-and-publishing.md)).

## Risk
Medium

## Problem statement
Users need a concise, reviewable way to define an ETL graph that supports:
- deterministic wiring of jobs,
- safe retries (at-least-once),
- explicit publishing of user-facing datasets,
- source triggers and reactive processing.

Constraints:
- DAG YAML is a **public surface** and must remain stable and minimal.
- Defaults must exist so most jobs can omit tuning knobs.
- The scheduler assumes at-least-once; the DAG format must make idempotency requirements explicit.

## Goals
- Provide a clear YAML schema that can be validated deterministically.
- Keep runtime surface area small for v1:
  - `lambda` for sources and untrusted user code (platform-managed runner),
  - `ecs_platform` for trusted platform operators,
  - `dispatcher` for internal/no-op control jobs.
- Make idempotency requirements explicit for `append` outputs (`unique_key`).
- Support both source-triggered and reactive jobs.
- Support publishing selected outputs as named datasets via the dataset registry.

## Non-goals
- Arbitrary dynamic DAG generation at runtime.
- Implicit dependency inference from naming conventions.
- Per-job IAM policy definitions in YAML.
- Exactly-once execution.

## Public surface changes
- Config semantics: DAG YAML schema (this document).
- Persistence: DAG versions and the active mapping (see [orchestration.md](../architecture/data_model/orchestration.md)).
- Intentionally not supported: per-job secret-slot leasing; implicit joins/fan-in behaviors.

## Architecture (C4) - Mermaid-in-Markdown only

```mermaid
flowchart LR
  U[User] -->|upload DAG YAML| API[User API contracts]
  API -->|validate + store| DISP[Dispatcher]
  DISP -->|persist| PS[(Postgres state: dag_versions)]
  DISP -->|enqueue tasks| Q[SQS task queues]
  Q -->|poll| EXEC[Executors: Lambda runner / ECS platform]
```

## Proposed design

### YAML schema (v1)

Top-level keys:

| Key | Required | Description |
|-----|----------|-------------|
| `name` | ✅ | DAG name (used for versioning and navigation) |
| `defaults` | | Optional default values applied to jobs when omitted |
| `jobs` | ✅ | List of job definitions |
| `publish` | | Optional dataset registry entries (metadata-only) |

Notes:
- Unknown fields MUST be rejected.
- Reserved fields MUST be rejected (see below).

```yaml
name: monad_hot_to_parquet

defaults:
  heartbeat_timeout_seconds: 60
  max_attempts: 3

jobs:
  - name: block_follower
    activation: source
    runtime: ecs_platform
    operator: block_follower
    source:
      kind: always_on
    config:
      chain_id: 10143
      rpc_pool: monad
      start_block: 1000000
      emit_strategy: per_update
    outputs: 2
    update_strategy: replace

  - name: block_range_aggregate
    activation: reactive
    runtime: ecs_platform
    operator: range_aggregator
    execution_strategy: PerUpdate
    inputs:
      - from: { job: block_follower, output: 0 }
    outputs: 1
    config:
      cursor_column: block_number
      range_size: 10000
    update_strategy: append
    unique_key: [dedupe_key]
    timeout_seconds: 60

  - name: parquet_compact
    activation: reactive
    runtime: ecs_platform
    operator: parquet_compact
    execution_strategy: PerPartition
    inputs:
      - from: { job: block_range_aggregate, output: 0 }
    outputs: 1
    config:
      chain_id: 10143
      dataset: blocks
      finality_depth_blocks: 100
      chunk_size: 10000
      delete_after_compact: true
    update_strategy: replace
    timeout_seconds: 1800

publish:
  blocks_parquet:
    from: { job: parquet_compact, output: 0 }
```

### Reserved fields (not supported in v1)

The following YAML fields are reserved for future backpressure controls and are **not supported in v1**.

- `max_queue_depth`
- `max_queue_age`
- `backpressure_mode`

Deploy/validation MUST reject configs that set these fields. This prevents “configs that look supported but are ignored.”

The following publish entry fields are reserved and are not supported in v1:
- `publish.*.storage`
- `publish.*.write_mode`
- `publish.*.schema`

Deploy/validation MUST reject publish entries that set these fields.

### Job fields (reference)

| Field | Required | Description |
|-------|----------|-------------|
| `name` | ✅ | Unique job name within the DAG |
| `activation` | ✅ | `source` or `reactive` |
| `runtime` | ✅ | `lambda`, `ecs_platform`, or `dispatcher` |
| `operator` | ✅ | Operator implementation identifier |
| `outputs` | ✅ | Number of outputs exposed as `output[0..N-1]` for wiring and publishing |
| `inputs` | reactive | Upstream edges (`from: {job, output}` or `from: {dataset: dataset_name}`), optionally with `where` |
| `execution_strategy` | reactive | `PerUpdate` or `PerPartition` |
| `idle_timeout` | reactive | Optional idle timeout for reactive jobs (for example `5m`) |
| `update_strategy` | reactive | `append` or `replace` |
| `unique_key` | if append | Required when `update_strategy=append` - columns used for idempotent upserts |
| `source` | source | Source config: `kind`, `schedule`/`webhook_path`, etc. |
| `bootstrap` | source | Optional one-time bootstrap actions (v1: `reset_outputs`) |
| `secrets` | | Logical secret names required by the operator |
| `scaling.max_concurrency` | | Optional cap on concurrently Running tasks for this job (limits parallelism) |
| `timeout_seconds` | | Hard execution timeout (platform-enforced) |
| `heartbeat_timeout_seconds` | | Heartbeat timeout used to detect dead attempts |
| `max_attempts` | | Maximum retry attempts (defaults to `defaults.max_attempts`) |
| `config` | | Operator-specific config |
| `udf` | | UDF bundle reference (required for `operator: udf` and any operator that executes user bundles) |

Notes:
- Trust is determined by the operator, not by the compute primitive. Treat `runtime: lambda` jobs as untrusted by default.
- Untrusted user code executes via the platform-managed `runtime: lambda` runner. See [udf.md](udf.md) and [operators/udf.md](operators/udf.md).
- Bootstrap actions run once when a `dag_version` becomes active and are not re-run on ordinary restarts.

### Input filters (`where`)
Reactive jobs may narrow upstream inputs with a `where` clause. The filter is applied by the platform (not user code) and must be safe/validatable.

Example:

```yaml
inputs:
  - from: { dataset: address_labels }
    where:
      chain_id: 1
      label_type: "cex"
```


### UDF jobs (optional)

Some jobs execute **user-defined bundles** (untrusted code). In v1 this is supported only with `runtime: lambda` (the platform-managed UDF runner).

- Use `operator: udf` for a generic "run this bundle" job (see [operators/udf.md](operators/udf.md)).
- Some built-in operators (for example `alert_evaluate`) require an `udf` block to supply the user logic (see [operators/alert_evaluate.md](operators/alert_evaluate.md)).

Add an `udf` block to the job:

```yaml
udf:
  bundle_id: "<bundle-id>"
  entrypoint: "trace.handler"
```

Notes:
- A single DAG may mix languages by referencing different bundles.
- Bundle language (node/python/rust) is recorded at upload time; Dispatcher selects the appropriate Lambda runner.
- For Rust custom runtime bundles, `entrypoint` is ignored (the bundle's `bootstrap` is executed).

Constraints:
- UDF jobs MUST NOT request `secrets`.
- UDF jobs MUST read only via Query Service and write only via task-scoped APIs.
- Use `update_strategy: append` + `unique_key` for idempotent sinks.


V1 constraints:
- Filters are simple equality matches on a small allowlist of fields for the referenced dataset/operator.
- No arbitrary SQL in DAG config.

### Publish fields (optional)
Publishing registers a job output as a named dataset.

In v1, publishing is metadata-only: it registers a name for discovery and authz. It does not change execution or storage behavior.
See [ADR 0008](../adr/0008-dataset-registry-and-publishing.md).

| Field | Required | Description |
|-------|----------|-------------|
| `from` | ✅ | `{job, output}` reference to publish |

See:
- [ADR 0008](../adr/0008-dataset-registry-and-publishing.md)
- [ADR 0006](../adr/0006-buffered-postgres-datasets.md)

### Defaults and overrides
- Values in `defaults` apply to all jobs unless overridden at job level.
- Backpressure is owned by the Dispatcher. In v1, per-job backpressure fields are reserved and deploy validation rejects configs that set them (see "Reserved fields" above).

### Deployment semantics
Deployment, rematerialization, cutover, and rollback invariants live in [dag_deployment.md](../architecture/dag_deployment.md).
This spec defines the DAG YAML surface only.

## Contract requirements
- The platform MUST validate DAG YAML before accepting it and return actionable errors.
- The YAML format MUST remain backwards compatible within a major version.
- Jobs with `update_strategy=append` MUST declare `unique_key`; the sink must upsert on that key.
- Untrusted runtimes MUST NOT be granted direct Postgres access; reads go through Query Service.

## Security considerations
- Threats: privilege escalation via config; cross-org dataset reads; secret leakage via UDF runtime.
- Mitigations:
  - strict validation + allowlisted operators/runtimes,
  - authz enforced by backend JWT verification + DB membership checks,
  - task-scoped capability tokens for untrusted execution,
  - secrets injected only into trusted runtimes.
- Residual risk: bad `unique_key` choices can cause duplicates or dropped rows; mitigate with operator-specific validation where possible.

## Alternatives considered
- Allow per-job IAM policies in YAML.
  - Why not: turns YAML into a security policy language and creates operational sprawl.
- Allow arbitrary SQL predicates in `where`.
  - Why not: injection risk; hard to validate; becomes an unbounded query feature.

## Acceptance criteria
- Tests:
  - YAML validation catches missing required fields and invalid references.
  - `append` jobs without `unique_key` are rejected.
  - Publishing updates the registry mapping and is visible to Query Service.
  - Deploying an unchanged YAML does not create a new DAG version.
- Observable behavior:
  - DAG deploy creates a new immutable version; scheduling uses the active version.
