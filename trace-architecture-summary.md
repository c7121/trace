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
- Current preferred idempotency approach (“Option B”): dedupe by something like `(producer_task_id, dataset_id, cursor|partition_key)`; note to reconsider if it becomes dicey.
- **Source runs** should have a durable identifier used as `producer_task_id`; source restarts behave like retry of the same source run to preserve idempotency.

## DAG deploy, rematerialization, and rollback

- Deploy/rematerialize should be **non-destructive**: build new versions, then cut over; keep old versions for fast rollback.
- Prefer **atomic cutover** over progressive cutover.
- It’s acceptable to serve **old (stale-but-consistent) outputs** until the rematerialization finishes and the atomic cutover occurs.
- Rematerialization scope is **changed jobs + downstream subgraph** (not necessarily whole DAG).
- Separate config into **materialization-affecting** vs **execution-only** properties; runtime changes are materialization-affecting.
- `execution_strategy` is on the incoming side (events→tasks); if it changes, we likely rematerialize, but allow a deploy-time override (“don’t rematerialize”) if the user explicitly opts out.
- Avoid interpreting `execution_strategy: Bulk` as “Dispatcher coalesces events”; batching/compaction should be explicit in the DAG (e.g., Aggregator/Splitter).
- **Query safety**:
  - Query Service resolves `dataset_id -> dataset_version` once at query start and pins that mapping for the duration of the query (no “moving target”).
  - Deploy cutover updates the whole affected `dataset_id -> current_version` pointer set **atomically** (single Postgres transaction).
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

- `dataset_id` is a **user-defined opaque string** (Trace should not parse semantics out of it). Docs can recommend conventions.
- `dataset_id` is intended to be **globally unique** to support cross-DAG reads in v2.
- Cross-DAG **shared reads** are a supported use case.
- Cross-DAG **shared writes** are not: enforce “single producing DAG active per `dataset_id`” (DAG-level ownership), while allowing multiple operators within a DAG to participate in a pipeline that ultimately produces a dataset.

## Materialization boundaries (lazy vs persisted)

- “Lazy dataframe” mental model: DAG may contain logical transforms; not every node must persist a table/file.
- For v1, allow **virtual/lazy nodes only for declarative transforms** (SQL/query-spec composition).
- Any non-declarative operator (Rust/UDF/etc.) is a **materialization boundary**.
- v1 Query Service should **not** auto-materialize/cached-persist virtual datasets while serving reads (simplicity).

## Side effects (alerts/pages)

- Side effects are in scope (alerts/pages).
- We must avoid re-firing historical pages during rematerialization/backfills; this needs explicit semantics (see `remaining-questions.md`).

## Doc + ADR edits to apply (next session)

- Update `docs/readme.md` to reflect:
  - SQS payload is `task_id` only; workers fetch via internal API.
  - Explicit `/internal/events` vs `/internal/task-complete`.
  - Postgres-as-queue framing and backpressure/buffering story.
  - Deploy/rematerialize: atomic cutover + fast rollback.
- Update `docs/architecture/contracts.md` to align worker/internal API contracts with:
  - task fetch by id, cancellation/cooperative abort, event emission/idempotency fields.
- Update `docs/capabilities/dag_configuration.md` to document:
  - `dataset_id` as opaque string + recommended naming conventions (not enforced).
  - execution_strategy is incoming-side (events→tasks).
  - explicit materialization boundaries; virtual SQL nodes vs materialized nodes.
  - Aggregator/Splitter operator patterns for compaction/batching.
- Update `docs/capabilities/udf.md` to clearly state:
  - wrapper is the security boundary; UDF code cannot access `/internal/*`.
- ADR work:
  - Add an ADR for “materialization boundaries + Postgres dataset modes + query pinning + atomic cutover/rollback”.
  - Reconcile/extend any existing ADRs that overlap (e.g., buffered Postgres datasets) with the above decisions.

## Next session

- Answer the remaining questions in `remaining-questions.md`, then apply doc/ADR patches.
