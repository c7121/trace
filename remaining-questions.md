# Remaining architecture questions (next session)

This is the list of unresolved questions to confirm before we edit docs/ADRs.

## 1) Side effects (alerts/pages) + rematerialize/rollback

- What is the intended **mechanism** for alerts/pages: a sink operator that calls PagerDuty/Slack directly, or an “alerts dataset” plus a notifier/consumer?
- Is alerting explicitly **two-stage**:
  - `alert_evaluate` produces `alert_events` (data)
  - `alert_deliver` produces `alert_deliveries` (work items)
  - a separate **Delivery Service** leases `alert_deliveries` and performs side effects (PagerDuty/Slack)?
- What is the **idempotency key** for side-effect actions (e.g. `alert_rule_id + dataset_id + cursor`, something else)?
- How do we prevent **backfill/rematerialize** from paging on “old” conditions:
  - `max_event_age`/time-window gating at the notifier?
  - an explicit `mode: live|backfill` flag propagated through tasks/events?
  - something else?
- If a deploy is rolled back, should side-effect operators run under the new DAG version at all, or be **paused** until cutover is stable?

## 2) Dataset identity + storage mapping

- `dataset_id` is an opaque string: what are the **allowed characters/length** and the canonical normalization rules (case sensitivity, escaping)?
- How is `dataset_id` mapped to physical storage across backends (parquet paths, Postgres identifiers): direct embedding, escaping, hashing, or “registry table” indirection?
- Do we maintain a global **dataset registry table** (per org) that stores dataset metadata + storage location + current producer, and is populated/validated at deploy time (single producer enforcement)?
- What is the exact shape of `dataset_version` (UUID? `(dag_version, epoch, partition)`?), and how does it relate to atomic cutover/rollback?

## 3) Virtual (SQL) nodes + Query Service

- How should “virtual SQL transforms” be represented in DAG config (e.g., `runtime: sql` + `query` + inputs)?
- Clarify **Query Service vs Query Operator** responsibilities: small queries inline vs delegate to a standard operator for heavier work; query operator follows the normal worker contract and is swappable (DuckDB/Athena/Trino/etc.).
- Are cross-backend queries in scope for v1 (parquet + Postgres in one query), or do we restrict to a single execution engine/backend per query?

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

- Where is Aggregator durable state stored (Postgres table schema + key): `{dataset_id, cursor_start, cursor_end, count, updated_at, ...}`?
- What is the manifest event schema for a batch range (minimum required fields)?
- When do we actually need the Splitter (inverse operator) in v1 vs later (parallelism, fan-out, “stream of batches”)?

## 6) Execution strategy semantics (events → tasks)

- Decision: Dispatcher does **not** coalesce upstream events; keep “1 event → 1 task”.
- If a job needs “bulk/compaction” behavior, model it explicitly in the DAG (e.g., Aggregator/Splitter) or have the source emit coarser-grain events.
- Open: keep `execution_strategy: Bulk` in the schema at all? If yes, redefine/rename so it doesn’t imply Dispatcher coalescing.

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
