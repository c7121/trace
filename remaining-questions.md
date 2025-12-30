# Remaining architecture questions (follow-ups)

This is the list of unresolved questions to confirm and/or capture as ADRs. Docs have begun to be updated; remaining decisions should be reconciled into `docs/` as they’re answered.

## 1) Side effects (alerts/pages) + rematerialize/rollback

- Decision: operators/UDFs are **not permitted to communicate with the outside world**; alerts/pages must be delivered via a platform service.
- Decision: keep delivery routing as **operators**, and keep delivery side effects in a platform **Delivery Service**:
  - `alert_evaluate` produces `alert_events` (audit: “would have alerted”)
  - one or more downstream routing operators produce `alert_deliveries` (work items; filtered by severity/channel/etc.)
  - Delivery Service leases `alert_deliveries`, sends externally, and updates delivery status
- Decision: use a **single shared `alert_deliveries` table** (or dataset) and a Delivery Service leasing loop; scale by running more Delivery Service instances before sharding by channel/destination.
- Decision: side-effect idempotency keys are part of the **operator contract** and are assigned by the operator (must be deterministic across retries).
- Decision: alert detectors emit an explicit **contextual event time** (e.g. `block_timestamp`), and user-configurable routing operators gate delivery creation by per-alert/route config like `max_delivery_age` (still record “would have alerted” rows even if delivery is suppressed).
- Decision: no `mode: live|backfill` in v1; rely on contextual time-gating + idempotency to avoid “re-firing” during rematerialize/backfill.
- If a deploy is rolled back, should side-effect operators run under the new DAG version at all, or be **paused** until cutover is stable?
- Decision: deploy edits/rollbacks do **not** cancel pending `alert_deliveries`; if it made it into the deliveries queue, it should be sent (subject to `max_delivery_age`/`event_time` gating and idempotency).

## 2) Dataset identity + storage mapping

- `dataset_name` is a user-defined string: what are the **allowed characters/length** and the canonical normalization rules (case sensitivity, escaping)?
- How is `dataset_name` mapped to physical storage across backends (parquet paths, Postgres identifiers): direct embedding, escaping, hashing, or “registry table” indirection (vs always using `dataset_uuid` for physical identifiers)?
- Do we maintain a global **dataset registry table** (per org) that stores dataset metadata + storage location + current producer, and is populated/validated at deploy time (single producer enforcement)?
- Permission model for datasets: do we attach ACLs at the dataset level (e.g., `read_roles`, `write_roles`, `owner_org_id`), and do DAGs/users interact with datasets via `{org_id, role, dag_id}` scoped identities?
- Decision: DAG deploy/trigger permission implies permission to read **and** overwrite/create datasets produced by that DAG (DAG permission ⇒ dataset read+write for produced datasets).
- Decision: datasets have independent `read_roles` for non-producers (shared reads) while producer DAG deploy/trigger permission still implies read+write.
- Decision: dataset `read_roles` is managed via admin (registry) controls, not via DAG config.
- Decision: the dataset registry is authoritative for resolving `dataset_name -> dataset_uuid -> {storage backend, location}` (not purely naming-convention derived).
- Decision: yes — distinguish:
  - `dataset_name` (human-readable, user-defined string; unique per org) used in registry + most APIs, and
  - `dataset_uuid` (system-generated UUID primary key) used internally and in storage paths to avoid escaping issues.
- Decision: the registry should link datasets back to their producer (`dag_id`, `job`, `output_index`) for navigation and “single producer” enforcement.
- Decision: DAG YAML wiring can stay index-based; user-visible datasets are created via a top-level `publish:` section that maps `{job, output_index} -> dataset_name`.
- Decision: `dataset_version` is a system-generated UUID; metadata lives in a `dataset_versions` table and `dataset_current` pointers are swapped atomically for cutover/rollback.
- Decision: publishing is **metadata-only** (registry update/aliasing) and does not trigger rematerialization/backfill or change how the DAG runs.
- Backlog: add **snapshot publishes** (pinned/immutable aliases) for “read as-of”; simplest shape is a one-shot/user-triggered action (or operator) that creates a new registry entry pointing to the current `dataset_version` and then never auto-advances.

## 3) Virtual (SQL) nodes + Query Service

- How should “virtual SQL transforms” be represented in DAG config (e.g., `runtime: sql` + `query` + inputs)?
- Clarify **Query Service vs Query Operator** responsibilities: small queries inline vs delegate to a standard operator for heavier work; query operator follows the normal worker contract and is swappable (DuckDB/Athena/Trino/etc.).
- Are cross-backend queries in scope for v1 (parquet + Postgres in one query), or do we restrict to a single execution engine/backend per query?
- Decision: Query Service only exposes **published** datasets (from `publish:` / registry); internal unpublished edges are not queryable.

## 4) Deploy/rematerialize mechanics + buffering

- During `POST /v1/dags` with rematerialization: for new incoming events, do we
  - keep creating tasks against the **old** active DAG until cutover, or
  - start creating tasks against the **new** DAG immediately but buffer dispatch, or
  - something else?
- What is the rule for choosing the “safe cutover line” in the DAG (where buffering is allowed without downstream corruption)?
- Backpressure nuance: sources keep emitting; “pause” means Dispatcher holds tasks in Postgres and stops pushing to SQS (valve is the Dispatcher, not sources).
- For buffered Postgres datasets: is it **one sink per dataset** (one SQS queue + one sink Lambda per dataset) or a shared sink service?
- Clarify `scaling.mode: steady|backfill` semantics (concurrency profile vs priority).
- Retention/GC: how long do we keep old versions for rollback, and what triggers cleanup?

## 5) Aggregator/Splitter operator details (compaction/batching)

- Where is Aggregator durable state stored (Postgres table schema + key): `{dataset_uuid, cursor_start, cursor_end, count, updated_at, ...}`?
- What is the manifest event schema for a batch range (minimum required fields)?
- When do we actually need the Splitter (inverse operator) in v1 vs later (parallelism, fan-out, “stream of batches”)?

## 6) Execution strategy semantics (events → tasks)

- Decision: Dispatcher does **not** coalesce upstream events; keep “1 event → 1 task”.
- If a job needs “bulk/compaction” behavior, model it explicitly in the DAG (e.g., Aggregator/Splitter) or have the source emit coarser-grain events.
- Decision: remove `execution_strategy: Bulk` from the schema (or make it invalid) — “bulk” behavior is expressed explicitly via Aggregator/Splitter (or by source event granularity), not by Dispatcher semantics.

## 7) Worker contracts (internal APIs)

- Confirm `/internal/task-fetch` response payload shape (operator runtime/config, input event, dataset refs, idempotency context).
- Confirm cooperative cancel semantics: which calls return “canceled”, and what is the wrapper’s stop/ack behavior?
- Lambda wrapper specifics:
  - does the invocation carry full payload or only `task_id` (then fetch)?
  - disable Lambda built-in retries; Dispatcher owns retries uniformly across runtimes.
  - how are Lambda timeouts handled (wrapper heartbeat/lease expiry + Dispatcher reaper)?
- Runtime registry/backlog: document likely runtime variants (`lambda`, `lambda_rs`, `lambda_py`, `ecs_rust`, `ecs_python`, ...).

## 8) Security, secrets, and roles

- Secrets scoping: separate **platform** vs **user** secrets namespaces; workers must not get platform secrets by default.
- Secret Writer Service: write-only, role-scoped (prevents read-back exfiltration).
- Role model: consolidate `users.role` vs `org_roles`/memberships into one system.

## 9) Schema / model cleanup

- Clarify whether `tasks.input_versions JSONB` vs `task_inputs` table are both required; if yes, document distinct purposes (fast-path vs lineage/memoization).

## 10) Data versioning + invalidations

- When a job reprocesses due to an upstream invalidation, does it also **cascade invalidations** to its own outputs?
- If we have `update_strategy` variants:
  - `replace`: should it always emit invalidations downstream for the replaced range?
  - `append`: should it rely on dedupe (`unique_key`) and allow orphaned rows to remain (auditable)?

## 11) Dispatcher spec gaps

- Dispatcher concurrency model (v1): single process vs sharded/workers; how we describe HA/restarts (ECS restart, leader election, etc.).
- Event emission API ergonomics: do we support both
  - `POST /internal/events` mid-task, and
  - “final events” bundled with `/internal/task-complete`?

## 12) PII handling (deferred)

- PII tagging, column-level policies, and enforcement is explicitly deferred for a future pass.
