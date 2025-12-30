# Trace architecture summary (Codex, current understanding)

This file captures what I believe we agreed on during the architecture walkthrough, plus the doc/ADR edits to make next.

## System shape (high level)

- **Postgres is the system database** (durable queue/state/metadata). We’re “NiFi-like”: Postgres is a slow queue; SQS is a dispatch/notification layer.
- **Dispatcher** writes tasks/state to Postgres, meters work out to SQS.
- Dispatcher should not “silently” change DAG semantics (e.g., coalescing/compacting work); batching/compaction should be modeled explicitly in the DAG via operators.
- **Workers** execute tasks via a **wrapper** process that enforces the platform contract + security boundary around UDF/operator code.
- **Query Service** reads “current” dataset versions and can evaluate virtual (SQL) datasets.

## Interfaces / contracts

- **SQS payload**: `task_id` only.
- Worker uses an **internal Dispatcher API** (e.g. `/internal/task-fetch`) to fetch task details by `task_id` (read-only from the worker’s perspective).
- `/internal/*` endpoints are for **platform components** (worker wrapper, operator runtimes), not end users.
- **UDF/operator code must not be able to call `/internal/*`**; the wrapper is the protection boundary.

## Events, tasks, and delivery semantics

- One incoming **event** triggers one **task** (1→1 on the incoming side).
- One task may emit **many outgoing events** (batch→stream style fan-out).
- Event emission is **explicit**: use `POST /internal/events`.
- `POST /internal/task-complete` is lifecycle-only and happens **after** all intended events are successfully emitted.
- Delivery is **at-least-once with idempotency**.
- Current preferred idempotency approach (“Option B”): dedupe by something like `(producer_task_id, dataset_uuid, cursor|partition_key)`; note to reconsider if it becomes dicey.
- **Source runs** should have a durable identifier used as `producer_task_id`; source restarts behave like retry of the same source run to preserve idempotency.

## DAG deploy, rematerialization, and rollback

- Deploy/rematerialize should be **non-destructive**: build new versions, then cut over; keep old versions for fast rollback.
- Prefer **atomic cutover** over progressive cutover.
- It’s acceptable to serve **old (stale-but-consistent) outputs** until the rematerialization finishes and the atomic cutover occurs.
- Rematerialization scope is **changed jobs + downstream subgraph** (not necessarily whole DAG).
- Separate config into **materialization-affecting** vs **execution-only** properties; runtime changes are materialization-affecting.
- Incoming side is fixed to **1 event → 1 task** for v1; “bulk/compaction” behavior is expressed explicitly in the DAG (Aggregator/Splitter) or by source event granularity (no Dispatcher coalescing).
- **Query safety**:
  - Query Service resolves `dataset_uuid -> dataset_version` once at query start and pins that mapping for the duration of the query (no “moving target”).
  - Deploy cutover updates the whole affected `dataset_uuid -> current_version` pointer set **atomically** (single Postgres transaction).
  - Rollback is a single operation that atomically restores the prior pointer set **and** restores the active `dag_version` (full control-plane + data-plane rollback).
- Rollback/task handling recommendation:
  - Stop leasing/dispatching tasks for the rolled-back (bad) `dag_version`.
  - Cancel queued tasks for that `dag_version` in Postgres.
  - Cooperative cancel for in-flight tasks via `/internal/task-fetch` / `/internal/heartbeat` returning “canceled”.
  - Ensure outputs are written to **versioned artifacts** so nothing from the bad version becomes “current” after rollback.

## Batching / compaction (EIP operators)

- Replace “planner” terminology with explicit EIP operators: **Aggregator** + **Splitter** (inverse of each other).
- For v1, Aggregator/Splitter are **normal operators** (ship fast). Backlog: consider “virtual operators” later (could still just be a small `lambda` operator that rewrites/augments a downstream query).
- For record-count batching (e.g., 10k-block parquet partitions): Aggregator emits a **deterministic batch manifest** with explicit boundaries.
- Cursor/ordering key is **user-defined in the DAG** (not necessarily “rows”); example is `block_height` driving transaction materialization.
- Batching is **event-driven** (per-unit events accumulate) and v1 assumes the stream is **ordered**.
- Range/window definition belongs to the DAG/operator (Aggregator), not the Dispatcher.
- Multi-chain is modeled as multiple source nodes (one ordered cursor per source); aggregate per source then join downstream.

## Dataset identity and ownership

- `dataset_name` is a **human-readable, user-defined string** (unique per org) used for discovery/navigation in the registry and most user-facing APIs. Docs can recommend conventions.
- `dataset_uuid` is a **system-generated UUID primary key** used internally and in storage paths to avoid escaping issues.
- Cross-DAG **shared reads** are a supported use case.
- Cross-DAG **shared writes** are not: enforce “single producing DAG active per `dataset_uuid`” (DAG-level ownership), while allowing multiple operators within a DAG to participate in a pipeline that ultimately produces a dataset.
- Likely permission model: datasets are scoped to an org and access is controlled via org roles; DAGs/users interact with datasets under `{org_id, role, dag_id}` scoped identities (exact ACL shape TBD).
- If you can deploy/trigger a DAG, you implicitly have permission to read **and** overwrite/create the datasets it produces (DAG permission ⇒ dataset read+write for produced datasets).
- Datasets can also expose `read_roles` to allow shared reads by non-producers (without granting DAG edit/deploy rights).
- `read_roles` should be managed via admin controls on the dataset registry (not embedded in DAG config).
- Dataset registry is the authoritative mapping for `dataset_name -> dataset_uuid -> {storage backend, location}` (not purely derived from naming conventions) and should link datasets back to their producer (`dag_id`, `job`, `output_index`) for navigation.
- DAG YAML primarily wires `node.output[i] -> other_node.input[j]` and does not require global dataset naming for every edge. User-visible datasets are created by a top-level `publish:` section that maps `{job, output_index} -> dataset_name`; ACLs remain admin-managed in the registry.
- Publishing is metadata-only: it registers/aliases an existing node output as a user-visible dataset in the registry (does not trigger rematerialization/backfill or change execution).
- Backlog: support **snapshot publishes** (pinned/immutable aliases) for “read this dataset as-of a point in time”; simplest shape is a one-shot/user-triggered action (or operator) that creates a new registry entry pointing to the current `dataset_version` and then never auto-advances.
- `dataset_version` should be an opaque system UUID; version metadata lives in `dataset_versions`, and `dataset_current` pointers are swapped atomically for deploy/rollback.
- A dataset’s materialization lifecycle is owned by its producing DAG; other DAGs can read/subscribe to it (shared reads) but do not “drive” the producer dataset’s materialization.

## Materialization boundaries (lazy vs persisted)

- “Lazy dataframe” mental model: DAG may contain logical transforms; not every node must persist a table/file.
- For v1, allow **virtual/lazy nodes only for declarative transforms** (SQL/query-spec composition).
- Any non-declarative operator (Rust/UDF/etc.) is a **materialization boundary**.
- v1 Query Service should **not** auto-materialize/cached-persist virtual datasets while serving reads (simplicity).
- Query Service should only expose **published** datasets (from `publish:` / registry). Unpublished internal edges are not directly queryable (publish a new dataset if needed).

## Side effects (alerts/pages)

- Side effects are in scope (alerts/pages).
- Operators/UDFs are not permitted to communicate with the outside world; side effects must be handled by a platform service (e.g., Delivery Service).
- Alert evaluation should record “would have alerted” durably (e.g., `alert_events`) even when delivery is suppressed.
- Alert detectors should emit an explicit **contextual event time** (domain timestamp like `block_timestamp`, not processing/created time).
- Staleness gating should be applied by user-configurable routing operators when creating `alert_deliveries` (e.g., `max_delivery_age` against contextual `event_time`); Delivery Service just sends pending deliveries.
- Avoid `live|backfill` modes in v1; rely on contextual time-gating + idempotency for delivery.
- Idempotency keys for alert deliveries are assigned by the alert operator(s) as part of the operator contract (deterministic across retries).
- Keep alert routing as operators: downstream operators can filter `alert_events` and write `alert_deliveries` for different destinations; only the Delivery Service performs external sends and updates delivery status.
- Use a single shared `alert_deliveries` table/dataset and scale Delivery Service horizontally via leasing (before introducing per-channel sharding).
- Deploy edits/rollbacks do **not** cancel pending deliveries: if a delivery is in `alert_deliveries` it should be sent (still subject to `max_delivery_age`/`event_time` gating and idempotency).

## Doc + ADR edits applied

- Updated `docs/readme.md` to align with the `task_id`-only dispatch contract and `dataset_uuid` event routing.
- Updated `docs/architecture/contracts.md` to use `dataset_uuid`, explicit `/internal/events`, and output indexing.
- Rewrote `docs/capabilities/dag_configuration.md` around index-wired edges + top-level `publish:` and removed `execution_strategy: Bulk` (use explicit Aggregator/Splitter operators).
- Updated alerting docs to match “operators route, Delivery Service sends” and added contextual `event_time` + `max_delivery_age` gating (`docs/capabilities/alerting.md`, `docs/architecture/adr/0004-alert-event-sinks.md`).
- Added dataset registry/publishing ADR: `docs/architecture/adr/0008-dataset-registry-and-publishing.md`.
- Updated data model docs to include the dataset registry + `dataset_uuid` usage (`docs/capabilities/orchestration.md`, `docs/architecture/erd.md`, `docs/architecture/data_versioning.md`).
- Updated operator docs and catalog; renamed `alert_deliver` → `alert_route` and added `range_aggregator` / `range_splitter`.

## Remaining follow-ups

- Add an ADR for atomic cutover/rollback + query pinning details (and reconcile with the existing `partition_versions`/`dataset_cursors` approach).
- Work through the remaining items in `remaining-questions.md` (buffering/cutover line, aggregator durable state schema, secrets/roles, etc.).
