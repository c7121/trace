# Trace architecture: canonical decisions (checklist)

This is the single source of truth for the architecture decisions captured during the Codex-assisted walkthrough.

Each checklist item is a decision. Mark it `[x]` only when it is reflected in the referenced docs.

## Canonical decisions

### System shape

- [x] **Postgres is the system database** (durable state/queue/metadata); SQS is a dispatch/notification layer for workers. Docs: `docs/readme.md`, `docs/capabilities/orchestration.md`
- [x] **Dispatcher** persists tasks/state to Postgres and meters work out to workers (SQS/ECS and/or direct invocation). Docs: `docs/readme.md`, `docs/capabilities/orchestration.md`
- [x] Dispatcher must not silently change DAG semantics (no implicit batching/compaction); batching/compaction is modeled explicitly via operators. Docs: `docs/capabilities/dag_configuration.md`, `docs/architecture/operators/range_aggregator.md`, `docs/architecture/operators/range_splitter.md`
- [x] **Workers** execute tasks via a **wrapper** (contract enforcement + security boundary around operator/UDF code). Docs: `docs/readme.md`, `docs/architecture/contracts.md`, `docs/standards/security_model.md`
- [x] **Query Service** resolves dataset identity + pinned versions and executes queries. Docs: `docs/architecture/query_service.md`, `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`

### Events, tasks, delivery

- [x] Incoming semantics (v1): **1 event → 1 task** (no dispatcher-side coalescing). Docs: `docs/architecture/data_versioning.md`, `docs/architecture/dag_deployment.md`
- [x] Outgoing: **1 task → many events** is normal (batch→stream fan-out). Docs: `docs/architecture/contracts.md`
- [x] Event emission is explicit. Docs: `docs/architecture/contracts.md`
  - [x] `POST /internal/events` for mid-task/progress/streaming events. Docs: `docs/architecture/contracts.md`
  - [x] `POST /internal/task-complete` for lifecycle completion and optional “final events” bundled atomically with completion. Docs: `docs/architecture/contracts.md`
- [x] Task completion happens only after all intended events have been accepted (either emitted earlier via `/internal/events` or included as “final events” on `/internal/task-complete`). Docs: `docs/architecture/contracts.md`
- [x] Delivery is **at-least-once with idempotency**; operators that create/update data must be idempotent. Docs: `docs/architecture/data_versioning.md`, `docs/architecture/contracts.md`
- [x] Preferred idempotency key shape (working assumption): dedupe by `(producer_task_id, dataset_uuid, cursor|partition_key|range)` (revisit if dicey). Docs: `docs/architecture/data_versioning.md`, `docs/capabilities/alerting.md`
- [x] Source runs should have a durable `producer_task_id` / run ID; source restarts behave like retry of the same run (preserve idempotency). Docs: `docs/architecture/contracts.md`
- [x] “Bulk” is not a Dispatcher feature: express batching/compaction explicitly via Aggregator/Splitter (or by source event granularity). Docs: `docs/capabilities/dag_configuration.md`, `docs/architecture/operators/range_aggregator.md`, `docs/architecture/operators/range_splitter.md`
- [x] `execution_strategy: Bulk` is not supported; model “bulk” explicitly via Aggregator/Splitter. Docs: `docs/capabilities/dag_configuration.md`

### Worker + Dispatcher contracts (internal APIs)

- [x] `/internal/*` endpoints are for platform components (worker wrapper, operator runtimes), not end users. Docs: `docs/architecture/contracts.md`
- [x] Operator/UDF code must not be able to call `/internal/*`; the wrapper is the protection boundary. Docs: `docs/architecture/contracts.md`, `docs/standards/security_model.md`
- [x] **SQS payload**: `task_id` only (thin message). Docs: `docs/architecture/contracts.md`, `docs/readme.md`
- [x] For SQS/ECS workers: wrapper calls `GET /internal/task-fetch?task_id=...` to fetch the full task payload (read-only from the worker’s perspective). Docs: `docs/architecture/contracts.md`, `docs/readme.md`
- [x] For `runtime: lambda` jobs: Dispatcher invokes Lambda with the **full task payload** (same shape as `/internal/task-fetch`), not just `task_id`. Docs: `docs/architecture/contracts.md`, `docs/readme.md`, `docs/capabilities/orchestration.md`
- [x] Lambda language runtimes: TypeScript/JavaScript, Rust, and Python. Docs: `docs/architecture/contracts.md`, `docs/capabilities/dag_configuration.md`, `docs/capabilities/alerting.md`
- [x] Cooperative cancel: `/internal/task-fetch` can return `status: "Canceled"`; wrapper exits without running operator code and reports `/internal/task-complete` as canceled. Docs: `docs/architecture/dag_deployment.md`, `docs/architecture/contracts.md`
- [x] Lambda retries/timeouts are owned by Dispatcher (disable Lambda built-in retries). Docs: `docs/architecture/contracts.md`, `docs/readme.md`
- [x] Attempt-gated `/internal/events` + `/internal/task-complete` (reject stale attempts once a newer attempt starts). Docs: `docs/architecture/contracts.md`
  - [x] Record `started_at` per attempt; reaper marks timeout and schedules retry until `max_attempts`. Docs: `docs/capabilities/orchestration.md`
  - [x] Accept events/completion only for the current attempt. Docs: `docs/architecture/contracts.md`
  - [x] Accept late completion for the current attempt if no newer attempt has started and task is not `Canceled`/`Completed`. Docs: `docs/architecture/contracts.md`

### Dataset identity, registry, publishing

- [x] Distinguish `dataset_name` (human-readable, unique per org) vs `dataset_uuid` (system UUID, used internally + in storage). Docs: `docs/architecture/adr/0008-dataset-registry-and-publishing.md`, `docs/capabilities/orchestration.md`
- [x] Cross-DAG **shared reads** are supported. Docs: `docs/architecture/adr/0008-dataset-registry-and-publishing.md`
- [x] Cross-DAG **shared writes** are not supported by default in v1 (single producing DAG per `dataset_uuid`), except for explicitly-declared buffered sink datasets intended to be multi-writer within a DAG (e.g., `alert_events`). Docs: `docs/architecture/adr/0008-dataset-registry-and-publishing.md`, `docs/architecture/adr/0006-buffered-postgres-datasets.md`, `docs/capabilities/alerting.md`
- [x] A dataset’s materialization lifecycle is owned by its producing DAG; other DAGs can read/subscribe (shared reads) but do not “drive” the producer dataset’s materialization. Docs: `docs/architecture/adr/0008-dataset-registry-and-publishing.md`
- [x] `dataset_name` format (v1): regex `^[a-z][a-z0-9_]{0,127}$` (lower snake_case), max length 128. Docs: `docs/architecture/adr/0008-dataset-registry-and-publishing.md`
- [x] Physical storage identifiers always use `dataset_uuid` (and `dataset_version`), never `dataset_name`. Docs: `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`, `docs/architecture/query_service.md`
  - [x] S3 paths are UUID-based (recommend including `org_id` prefix): `.../org/{org_id}/dataset/{dataset_uuid}/version/{dataset_version}/...`. Docs: `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`
  - [x] Postgres physical tables are UUID-based; user-facing SQL uses views named `dataset_name`. Docs: `docs/architecture/query_service.md`
- [x] Registry is a mandatory system table and is authoritative for `dataset_name → dataset_uuid` + producer metadata; versioned storage locations are resolved via `dataset_versions` + DAG pointer sets. Docs: `docs/architecture/adr/0008-dataset-registry-and-publishing.md`, `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`, `docs/architecture/query_service.md`, `docs/capabilities/orchestration.md`
- [x] DAG YAML wiring is index-based; user-visible datasets are created via top-level `publish:` mapping `{job, output_index} -> dataset_name` (metadata-only). Docs: `docs/architecture/adr/0008-dataset-registry-and-publishing.md`, `docs/capabilities/dag_configuration.md`
- [x] Publishing is metadata-only (registry update/aliasing) and does not trigger rematerialization/backfill. Docs: `docs/architecture/adr/0008-dataset-registry-and-publishing.md`
- [x] Backlog: snapshot publish (pinned alias) for “read as-of” (one-shot/user-triggered). Docs: `docs/architecture/adr/0008-dataset-registry-and-publishing.md`

### Permissions (simple + safe defaults)

- [x] Dataset ACLs are registry-managed (via `datasets.read_roles`). Docs: `docs/capabilities/orchestration.md`, `docs/architecture/query_service.md`
- [x] If `read_roles` is empty: dataset is private to producing DAG deployers/triggerers plus org admins (no org-wide-by-default footguns). Docs: `docs/standards/security_model.md`
- [x] If `read_roles` is non-empty: those org roles get read access in addition to producing DAG + org admins. Docs: `docs/standards/security_model.md`
- [x] `read_roles` gates all reads: Query Service reads and DAG-to-dag reads via `inputs: from: {dataset: ...}`. Docs: `docs/architecture/query_service.md`, `docs/capabilities/dag_configuration.md`
- [x] DAG deploy/trigger permission implies permission to read and overwrite/create datasets produced by that DAG. Docs: `docs/standards/security_model.md`
- [x] Dataset `read_roles` is admin-managed (registry), not embedded in DAG config. Docs: `docs/capabilities/orchestration.md`, `docs/architecture/adr/0008-dataset-registry-and-publishing.md`
- [x] No “anti re-sharing” controls in v1. Docs: `docs/standards/security_model.md`

### Query Service + query ergonomics

- [x] Query Service attaches per-org dataset views so user SQL can stay `SELECT * FROM dataset_name`. Docs: `docs/architecture/query_service.md`
- [x] Query pinning: resolve `dataset_uuid → dataset_version` once at query start and pin for duration (no “moving target”). Docs: `docs/architecture/query_service.md`, `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`
- [x] v1 supports cross-backend reads (Postgres + S3/Parquet) via DuckDB, with interactive limits and batch mode. Docs: `docs/architecture/query_service.md`
- [x] v1 Query Service does not auto-materialize/cache intermediate datasets while serving reads. Docs: `docs/architecture/query_service.md`
- [x] v1 only exposes published datasets; unpublished internal edges are not directly queryable. Docs: `docs/architecture/query_service.md`, `docs/architecture/adr/0008-dataset-registry-and-publishing.md`
- [x] v1 does not introduce `runtime: sql` nodes; SQL transforms are normal operators. Docs: `docs/architecture/query_service.md`

### Deploy/rematerialize, buffering, rollback

- [x] Deploy/rematerialize is non-destructive: build new versions, then atomic cutover; keep old versions for fast rollback. Docs: `docs/architecture/dag_deployment.md`, `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`
- [x] Prefer atomic cutover over progressive/probabilistic cutovers. Docs: `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`
- [x] Cutover/rollback are single Postgres transactions that update the DAG’s active `dag_version` and the dataset-version pointer set. Docs: `docs/architecture/dag_deployment.md`, `docs/capabilities/orchestration.md`, `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`
- [x] Outputs are written as versioned artifacts (`dataset_version`); “current” is a pointer swap, not in-place mutation. Docs: `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`
- [x] Serve stale-but-consistent outputs until rematerialization finishes and cutover occurs. Docs: `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`
- [x] “From the edit onward” means edited job + transitive downstream dependents. Docs: `docs/architecture/dag_deployment.md`
- [x] Rematerialization scope is the edited job + downstream subgraph (not necessarily whole DAG). Docs: `docs/architecture/dag_deployment.md`
- [x] Separate job config into materialization-affecting vs execution-only fields; edits to materialization-affecting fields trigger rematerialization. Docs: `docs/architecture/dag_deployment.md`
- [x] During rematerialization: unchanged sources keep running/emitting events; Dispatcher buffers downstream work in Postgres and drains after cutover. Docs: `docs/architecture/dag_deployment.md`
- [x] Rollback/rollover pauses DAG processing; Delivery Service continues sending queued deliveries. Docs: `docs/architecture/dag_deployment.md`, `docs/capabilities/alerting.md`
- [x] Rollback cancels queued work for rolled-back `dag_version` and relies on cooperative cancel for in-flight tasks. Docs: `docs/architecture/dag_deployment.md`

### Buffered Postgres datasets

- [x] Buffered Postgres datasets use an SQS buffer + a platform-managed sink that writes Postgres and emits the upstream dataset event after commit. Docs: `docs/architecture/adr/0006-buffered-postgres-datasets.md`, `docs/architecture/contracts.md`
- [x] v1: one SQS queue per dataset and a sink consumer per dataset queue (shared code, per-dataset config) for isolation/backpressure. Docs: `docs/architecture/adr/0006-buffered-postgres-datasets.md`
- [x] Multi-writer buffered datasets are supported (multiple producers publish to the same buffer; sink relies on idempotency keys / unique constraints). Docs: `docs/architecture/adr/0006-buffered-postgres-datasets.md`, `docs/architecture/adr/0004-alert-event-sinks.md`

### Batching / compaction (Aggregator/Splitter, EIP)

- [x] Use explicit EIP operators: `range_aggregator` + `range_splitter` (inverse of each other). Docs: `docs/architecture/operators/range_aggregator.md`, `docs/architecture/operators/range_splitter.md`
- [x] v1: these are normal operators (not “virtual” planner nodes). Docs: `docs/architecture/operators/range_aggregator.md`
- [x] Range/window definition belongs to the DAG/operator (Aggregator), not the Dispatcher. Docs: `docs/architecture/operators/range_aggregator.md`, `docs/capabilities/dag_configuration.md`
- [x] Cursor/ordering key is user-defined in the DAG; batching is event-driven and v1 assumes ordered inputs. Docs: `docs/architecture/operators/range_aggregator.md`
- [x] Multi-source is modeled as multiple source jobs (one ordered cursor per source); aggregate per source, then join downstream. Docs: `docs/capabilities/dag_configuration.md`
- [x] Aggregator durable state lives in a platform-managed Postgres operator-state table keyed by `(org_id, job_id)` (and/or `input_dataset_uuid`). Docs: `docs/architecture/operators/range_aggregator.md`
- [x] Range manifest events include explicit range fields (`start`, `end`) in addition to `partition_key`. Docs: `docs/architecture/contracts.md`
- [x] Deterministic partitioning: Aggregator emits deterministic manifests with explicit boundaries (e.g., Parquet files per 10k blocks). Docs: `docs/architecture/operators/range_aggregator.md`, `docs/architecture/operators/cryo_ingest.md`

### Worker pools (Cryo backfill)

- [x] Ignore `scaling.mode` in v1; use `scaling.max_concurrency`. Docs: `docs/capabilities/dag_configuration.md`
- [x] DAG-level `worker_pools` define explicit arrays of worker “slots” (env + `secret_env`). Docs: `docs/capabilities/dag_configuration.md`, `docs/architecture/operators/cryo_ingest.md`
- [x] Jobs select a pool by name and set `scaling.max_concurrency`; effective concurrency is `min(max_concurrency, pool size)`. Docs: `docs/capabilities/dag_configuration.md`
- [x] Dispatcher leases one slot per running task and releases on completion/failure/timeout; wrapper injects env and fetches slot secrets. Docs: `docs/capabilities/dag_configuration.md`, `docs/standards/security_model.md`
- [x] Primary driver: Cryo backfills where each concurrent task needs its own RPC API key (and potentially multiple secrets). Docs: `docs/architecture/operators/cryo_ingest.md`

### Data versioning + invalidations

- [x] `dataset_version` is a system-generated UUID; `dataset_versions` + DAG pointer sets support atomic cutover/rollback. Docs: `docs/architecture/data_versioning.md`, `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`, `docs/capabilities/orchestration.md`
- [x] Retention/GC is admin-only: retain all `dataset_version`s until explicit purge (no automatic deletion in v1). Docs: `docs/architecture/data_versioning.md`, `docs/architecture/adr/0009-atomic-cutover-and-query-pinning.md`
- [x] Block-range Parquet partitions keep `{start}_{end}` in object key / filename (Cryo style), even if directory prefix is UUID/version-based. Docs: `docs/architecture/contracts.md`, `docs/architecture/data_versioning.md`, `docs/architecture/operators/cryo_ingest.md`
- [x] Invalidations cascade transitively through the DAG. Docs: `docs/architecture/data_versioning.md`
- [x] `update_strategy` semantics. Docs: `docs/architecture/data_versioning.md`
  - [x] `replace`: rewriting a scope/range always emits downstream invalidations for that scope/range. Docs: `docs/architecture/data_versioning.md`
  - [x] `append`: never delete; insert + dedupe by `unique_key` (orphans acceptable; retractions require `replace` or tombstones later). Docs: `docs/architecture/data_versioning.md`

### Side effects (alerts/pages)

- [x] Operators/UDFs do not communicate with the outside world; side effects are handled by a platform service. Docs: `docs/capabilities/alerting.md`, `docs/architecture/adr/0004-alert-event-sinks.md`, `docs/standards/security_model.md`
- [x] Routing is in the DAG; delivery is a platform Delivery Service. Docs: `docs/capabilities/alerting.md`, `docs/architecture/adr/0004-alert-event-sinks.md`
  - [x] Alert detectors produce `alert_events` (audit: “would have alerted”) as append-only facts. Docs: `docs/capabilities/alerting.md`
  - [x] Routing operators produce `alert_deliveries` work items (filters/destinations/staleness gating). Docs: `docs/capabilities/alerting.md`, `docs/architecture/operators/alert_route.md`
  - [x] Delivery Service leases `alert_deliveries`, performs external send, and updates status. Docs: `docs/capabilities/alerting.md`
- [x] Delivery idempotency keys must be deterministic across retries. Docs: `docs/capabilities/alerting.md`, `docs/architecture/adr/0004-alert-event-sinks.md`
- [x] Use a single shared `alert_deliveries` table and scale Delivery Service horizontally via leasing (before sharding). Docs: `docs/capabilities/alerting.md`
- [x] Alert detectors emit contextual `event_time` (e.g., `block_timestamp`). Docs: `docs/capabilities/alerting.md`
- [x] Staleness gating is configured in routing jobs (e.g., `max_delivery_age`) but still records “would have alerted” rows even if delivery suppressed. Docs: `docs/capabilities/alerting.md`, `docs/architecture/operators/alert_route.md`
- [x] No `mode: live|backfill` in v1; rely on contextual time-gating + idempotency. Docs: `docs/capabilities/alerting.md`
- [x] Deploy edits/rollbacks do not cancel pending deliveries: if it made it into `alert_deliveries`, it should be sent (subject to gating + idempotency). Docs: `docs/capabilities/alerting.md`

## Open items / ADR backlog (not yet decided)

- Define the exact `/internal/task-fetch` response payload shape (operator runtime/config, input event, dataset refs, idempotency context).
- Dispatcher HA/concurrency model (single process vs leader election/sharding).
- Secrets/roles work:
  - platform vs user secrets namespaces; workers must not get platform secrets by default
  - Secret Writer Service (write-only, role-scoped)
  - consolidate `users.role` vs org memberships into one model
- Schema cleanup: clarify `tasks.input_versions JSONB` vs `task_inputs` table responsibilities (fast-path vs lineage/memoization).
- Future: snapshot publishes and tombstones for `append`.
