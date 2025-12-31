# Trace architecture: canonical decisions + update plan

This is the single source of truth for the architecture decisions captured during the Codex-assisted walkthrough.
It supersedes `remaining-questions.md` (now an archive pointer).

## Canonical decisions

### System shape

- **Postgres is the system database** (durable queue/state/metadata). We’re “NiFi-like”: Postgres is the slow queue; SQS is a dispatch/notification layer.
- **Dispatcher** writes tasks/state to Postgres and meters work out to workers (SQS/ECS and/or direct invocation).
- Dispatcher must not silently change DAG semantics (no implicit batching/compaction); batching/compaction is modeled explicitly in the DAG via operators.
- **Workers** execute tasks via a **wrapper** that enforces the platform contract and is the security boundary around operator/UDF code.
- **Query Service** resolves dataset identity + current versions from the registry and executes queries with pinned versions.

### Events, tasks, delivery

- Incoming semantics (v1): **1 event → 1 task**. No dispatcher-side coalescing.
- Outgoing: **1 task → many events** is normal (batch→stream fan-out).
- Event emission is explicit:
  - `POST /internal/events` for mid-task/progress/streaming events.
  - `POST /internal/task-complete` for lifecycle completion **and** optionally “final events” bundled atomically with completion.
- Task completion happens only after all intended events have been accepted (either emitted earlier via `/internal/events` or included as “final events” on `/internal/task-complete`).
- Delivery is **at-least-once with idempotency**. Operators that can create/update data must be idempotent.
- Preferred idempotency key shape (current working assumption; revisit if dicey): dedupe by `(producer_task_id, dataset_uuid, cursor|partition_key|range)`.
- Sources should have a durable `producer_task_id` / source run ID; source restarts behave like retry of the same run to preserve idempotency.
- “Bulk” is not a Dispatcher feature: express batching/compaction explicitly via Aggregator/Splitter (or by source event granularity).
- `execution_strategy: Bulk` is not supported (remove/forbid it in DAG schema); model “bulk” explicitly via Aggregator/Splitter.

### Worker + Dispatcher contracts (internal APIs)

- `/internal/*` endpoints are for platform components (worker wrapper, operator runtimes), not end users.
- Operator/UDF code must not be able to call `/internal/*`; the wrapper is the protection boundary.
- **SQS payload**: `task_id` only (thin message).
- For SQS/ECS workers: wrapper calls `GET /internal/task-fetch?task_id=...` to fetch the full task payload (read-only from the worker’s perspective).
- For `runtime: lambda` jobs: Dispatcher invokes Lambda with the **full task payload** (operator/config/event context), not just `task_id` (still track status in Postgres; this is only about invocation payload shape).
- Cooperative cancel: `/internal/task-fetch` can return `status: "Canceled"`; wrapper exits without running operator code and reports `/internal/task-complete` as canceled (no outputs/events emitted).
- Lambda retries/timeouts:
  - Disable Lambda built-in retries; Dispatcher owns retries uniformly across runtimes.
  - Use **attempt-gated** `/internal/events` + `/internal/task-complete`:
    - Dispatcher records `started_at` per attempt.
    - Reaper marks timeout when `now() > started_at + timeout_seconds` and schedules retry until `max_attempts`.
    - Events/completion include attempt number and are accepted only if it matches the current attempt.
    - Late completion is accepted if it matches the **current** attempt and the task is not `Canceled`/`Completed` (prevents “attempt 3 succeeded late but wasn’t recorded”); older attempts are rejected once a newer attempt starts.

### Dataset identity, registry, publishing

- Distinguish:
  - `dataset_name`: human-readable, user-defined identifier (unique per org).
  - `dataset_uuid`: system-generated UUID primary key used internally and in physical storage names/paths.
- Cross-DAG **shared reads** are supported.
- Cross-DAG **shared writes** are not (v1): enforce “single producing DAG active per `dataset_uuid`”.
- A dataset’s materialization lifecycle is owned by its producing DAG; other DAGs can read/subscribe to it (shared reads) but do not “drive” the producer dataset’s materialization.
- `dataset_name` format (v1):
  - regex `^[a-z][a-z0-9_]{0,127}$` (lower snake_case), max length 128, unique per org.
- Physical storage identifiers always use `dataset_uuid` (and `dataset_version`), never `dataset_name`:
  - S3 paths are UUID-based (recommend including `org_id` prefix): `.../org/{org_id}/dataset/{dataset_uuid}/version/{dataset_version}/...`
  - Postgres physical tables are UUID-based (e.g., `dataset_{dataset_uuid}`); user-facing access is via Query Service views named `dataset_name` (and optionally Postgres views like `published.<dataset_name>`).
- Dataset registry is a **mandatory system table** and is authoritative for `dataset_name → dataset_uuid → {backend, location} (+ current dataset_version)`.
- Registry links datasets back to their producer (`dag_id`, `job`, `output_index`) for navigation and “single producer” enforcement.
- DAG YAML wiring stays index-based; user-visible datasets are created via a top-level `publish:` mapping `{job, output_index} -> dataset_name`.
- Publishing is **metadata-only** (registry update/aliasing) and does not trigger rematerialization/backfill.
- Backlog: “snapshot publish” (pinned alias) for “read as-of”; simplest is a one-shot/user-triggered action/operator that registers an alias pointing to the current `dataset_version` and then never auto-advances.

### Permissions (simple + safe defaults)

- Dataset ACLs are registry-managed:
  - If `read_roles` is NULL/empty: dataset is **private** to the producing DAG (users who can deploy/trigger that DAG) plus org admins (no org-wide-by-default footguns).
  - If `read_roles` is non-empty: those org roles get read access in addition to producing DAG + org admins.
- `read_roles` gates **all reads**: Query Service reads and DAG-to-dag reads via `inputs: from: {dataset: ...}`.
- DAG deploy/trigger permission implies permission to read **and** overwrite/create datasets produced by that DAG.
- Dataset `read_roles` is admin-managed (registry), not embedded in DAG config.
- No “anti re-sharing” controls in v1: if you grant someone read access to a dataset, they can use it in their DAGs and publish derived datasets.

### Query Service + query ergonomics

- Query Service resolves `dataset_name → dataset_uuid → {backend, location} (+ current dataset_version)` from the registry and attaches views so user SQL can stay `SELECT * FROM dataset_name`.
- Query pinning: Query Service resolves `dataset_uuid → dataset_version` once at query start and pins that mapping for the duration (no “moving target”).
- v1 supports cross-backend reads (Postgres + Parquet/S3 in one query) via DuckDB, with interactive limits and an escape hatch to a batch “query operator” for heavier work.
- v1 Query Service does not auto-materialize/cache intermediate datasets while serving reads (simplicity).
- v1 only exposes **published** datasets; unpublished internal edges are not directly queryable.
- v1 does **not** introduce “virtual SQL nodes” / `runtime: sql`; SQL transforms are expressed as normal operators (e.g., `operator: sql_transform`).

### Deploy/rematerialize, buffering, rollback

- Deploy/rematerialize is **non-destructive**: build new versions, then **atomic cutover**; keep old versions for fast rollback.
- Prefer atomic cutover over progressive/probabilistic cutovers (simpler, less error-prone).
- Cutover and rollback are **single Postgres transactions** that atomically update the “current” pointers for all affected datasets **and** the active `dag_version` (control-plane + data-plane together).
- Outputs are written as **versioned artifacts** (`dataset_version`); “current” is a pointer swap, not in-place mutation.
- It’s acceptable to serve old (stale-but-consistent) outputs until rematerialization finishes and cutover occurs.
- “From the edit onward” means the edited job plus everything that depends on it **transitively**.
- Rematerialization scope is the edited job + downstream subgraph (not necessarily the whole DAG).
- Separate job config into **materialization-affecting** vs **execution-only** fields; edits to materialization-affecting fields trigger rematerialization (v1: at minimum, runtime/operator/config that affects outputs).
- During a deploy that triggers rematerialization:
  - Unchanged source operators keep running/emitting events.
  - Dispatcher continues accepting `/internal/events` and persisting resulting work in Postgres, but **buffers** it (no worker dispatch) for the invalidated downstream subgraph until cutover, then drains buffered work using the new DAG version.
- Rollback/rollover pauses all DAG processing. Delivery Service is outside the DAG and continues sending any queued deliveries.
- Rollback cancels queued work for the rolled-back `dag_version` (and relies on cooperative cancel for in-flight tasks) so no outputs from the bad version can become “current”.

### Buffered Postgres datasets

- Buffered Postgres datasets use **one SQS queue per dataset** and a **dedicated sink consumer per dataset queue** (same code, deployed/configured per dataset) for isolation/backpressure.

### Batching / compaction (Aggregator/Splitter, EIP)

- Replace “planner” terminology with explicit EIP operators: **range_aggregator** + **range_splitter** (inverse of each other).
- v1: these are normal operators (ship fast); backlog “virtual operators” exploration later.
- Range/window definition belongs to the DAG/operator (Aggregator), not the Dispatcher.
- Cursor/ordering key is user-defined in the DAG (e.g., `block_height`); batching is event-driven and v1 assumes ordered inputs.
- Multi-source is modeled as multiple source jobs (one ordered cursor per source); aggregate per source, then join downstream.
- Aggregator durable state lives in a platform-managed Postgres operator-state table keyed by `(org_id, job_id)` (and/or `input_dataset_uuid` if needed).
- Range manifest events include explicit range fields (not just `partition_key`), e.g. `{dataset_uuid, partition_key, start, end}`.
- For deterministic “fixed-size partition” use cases (e.g., Parquet files per 10k blocks), Aggregator emits deterministic manifests with explicit boundaries.

### Worker pools (Cryo backfill)

- Remove/ignore `scaling.mode` in v1; use `scaling.max_concurrency`.
- DAGs can define one or more named **worker_pools** as explicit arrays of worker “slots” (not an array of secrets):
  - Each slot is a bundle of env vars + a mapping of env var → secret name(s) (and future per-worker metadata).
  - Jobs (primarily `ecs_*`) select a pool by name and set `scaling.max_concurrency`; effective concurrency is `min(max_concurrency, pool size)`.
  - Dispatcher leases one slot per running task and releases on completion/failure/timeout; wrapper injects env and fetches slot secrets.
  - v1 primary driver: Cryo backfills where each concurrent task needs its own RPC API key (and potentially multiple secrets) as a per-slot bundle.

### Data versioning + invalidations

- `dataset_version` is a system-generated UUID; metadata lives in `dataset_versions` and `dataset_current` pointers are swapped atomically for cutover/rollback.
- Retention/GC is **admin-only**: retain all `dataset_version`s until explicit purge (no automatic deletion in v1).
- Invalidations **cascade transitively**: when a job reprocesses due to an upstream invalidation, it invalidates its own outputs so downstream recomputes without needing awareness of upstream causes.
- `update_strategy` semantics:
  - `replace`: when a job rewrites a scope/range, it **always** emits invalidations downstream for that replaced scope/range.
  - `append`: append means **don’t delete** — the job only inserts and dedupes by `unique_key`; reprocessing may leave orphaned historical rows behind (auditable). If retractions/corrections are required, use `replace` (or a future tombstone design).

### Side effects (alerts/pages)

- Operators/UDFs are not permitted to communicate with the outside world; side effects are handled by a platform service.
- Keep delivery routing as operators + delivery as a platform service:
  - `alert_evaluate` produces `alert_events` (audit: “would have alerted”).
  - One or more routing operators produce `alert_deliveries` (work items; filtered by severity/channel/etc.).
  - Delivery Service leases `alert_deliveries`, sends externally, and updates delivery status.
- Delivery idempotency keys are part of the operator contract and must be deterministic across retries (operator assigns them; Delivery Service relies on them).
- Use a single shared `alert_deliveries` table/dataset and scale Delivery Service horizontally via leasing before sharding.
- Alert detectors emit a contextual `event_time` (e.g., `block_timestamp`).
- User-configurable routing operators gate delivery creation by config like `max_delivery_age` (still record “would have alerted” rows even if delivery is suppressed).
- No `mode: live|backfill` in v1; rely on contextual time-gating + idempotency.
- Deploy edits/rollbacks do not cancel pending deliveries: if it made it into `alert_deliveries`, it should be sent (subject to `max_delivery_age`/`event_time` gating and idempotency).

## Open items / ADR backlog

- Define the exact `/internal/task-fetch` response payload shape (operator runtime/config, input event, dataset refs, idempotency context).
- Dispatcher HA/concurrency model (single process vs leader election/sharding).
- Secrets/roles work:
  - platform vs user secrets namespaces; workers must not get platform secrets by default
  - Secret Writer Service (write-only, role-scoped)
  - consolidate `users.role` vs org memberships into one model
- Schema cleanup: clarify `tasks.input_versions JSONB` vs `task_inputs` table responsibilities (fast-path vs lineage/memoization).
- Future: snapshot publishes and tombstones for `append`.

## Update plan (staged, with review checkpoints)

Stage 0 (done): Collate decisions into this document and de-duplicate `remaining-questions.md`.

Stage 1: Align core contracts (you review after)
- Update `docs/architecture/contracts.md` to match: lambda full payload, attempt-gating, and “final events in task-complete”.
- Update `docs/architecture/dag_deployment.md` to match: transitive downstream invalidation, buffering during rematerialize, and atomic cutover/rollback semantics.

Stage 2: Align data/versioning + registry semantics (you review after)
- Update `docs/architecture/data_versioning.md` to match: invalidation cascade, `update_strategy` rules, admin-only retention, and where invalidation events live.
- Reconcile ADR 0009’s schema narrative with the concrete tables referenced in `docs/architecture/data_versioning.md` / `docs/capabilities/orchestration.md`.

Stage 3: Align DAG config + worker pools + query docs (you review after)
- Update `docs/capabilities/dag_configuration.md` to include DAG-level `worker_pools` + job references and remove/ignore `scaling.mode`.
- Update `docs/architecture/query_service.md` to match: name→uuid resolution, view attachment, and pinned versions.

Stage 4: Consistency sweep (you review after)
- `rg` for stale terminology/assumptions (`live|backfill` modes, `execution_strategy: Bulk`, `task_id`-only Lambda invocation, `scaling.mode`, etc.) and reconcile remaining contradictions across `docs/`.

After each stage, I’ll stop and ask you to review the diffs before moving on.
