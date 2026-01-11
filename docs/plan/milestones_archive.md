# Milestones archive

Detailed notes for completed milestones.

The canonical ledger and forward-looking plan lives in `milestones.md`.

Milestone notes are historical; current behavior is defined by `docs/architecture/*` and `docs/specs/*`.

## Milestone 8: Dispatcher extraction

Status: **complete** (tag: `ms/8`).

### Goal
Move the dispatcher HTTP router and background loops out of `harness/` into a reusable internal crate, without changing endpoint or DB semantics.

### Context links
- `docs/architecture/containers/dispatcher.md`
- `docs/architecture/contracts.md`
- `docs/architecture/task_lifecycle.md`

### Deliverables
- Add `crates/trace-dispatcher` containing the dispatcher router, handlers, outbox drainer, and lease reaper.
- Keep `harness/src/dispatcher.rs` as a thin wrapper that wires config + lite adapters and exposes the existing `DispatcherServer` API.
- Harness controls enable/disable of outbox and lease reaper loops.
- Reuse existing `trace-core` traits/adapters; add no new abstractions.

### STOP gate
- `cd harness && cargo test -- --nocapture`

---

## Milestone 9: Sink extraction

Status: **complete** (tag: `ms/9`).

### Goal
Move the buffer sink consumer (decode/validate/write + DLQ) out of `harness/` into a reusable internal crate, without changing DLQ or idempotency behavior.

### Context links
- `docs/adr/0004-alert-event-sinks.md`
- `docs/architecture/operations.md`
- `docs/specs/operators/alert_evaluate.md`

### Deliverables
- Add `crates/trace-sink` with the sink loop and message handler wired via `trace-core` `Queue`/`ObjectStore`.
- Keep `harness/src/sink.rs` as a thin wrapper that constructs lite adapters and calls into `trace-sink`.
- Bad batches remain fail closed: no partial DB writes and poison messages land in DLQ after retries.
- Reuse existing `trace-core` traits/adapters; add no new abstractions.

### STOP gate
- `cd harness && cargo test -- --nocapture`

---

## Milestone 10: RuntimeInvoker interface

Status: **complete** (tag: `ms/10`).

### Goal
Define a single `RuntimeInvoker` interface for "invoke untrusted UDF" with Lite and AWS implementations, without changing UDF behavior.

### Context links
- `docs/specs/udf.md`
- `docs/specs/operators/udf.md`
- `docs/architecture/contracts.md`

### Deliverables
- Add `trace_core::runtime::RuntimeInvoker` using `UdfInvocationPayload` as the request type.
- Implement Lite invocation by routing existing harness `FakeRunner` through the trait (no behavior changes).
- Add `trace_core::aws::AwsLambdaInvoker` behind the `aws` feature (compile-only at ms/10).
- Reuse existing contracts; add no new public APIs.

### STOP gate
- `cd harness && cargo test -- --nocapture`
- `cd crates/trace-core && cargo check --features aws`

---

## Milestone 11: Parquet dataset versions + safe Query Service attach

Status: **complete** (tag: `ms/11`).

### Goal
Make Parquet the canonical dataset artifact while keeping the SQL sandbox intact: untrusted SQL must **not** be able to reference file paths, URLs, or S3 keys directly.

This milestone introduces the minimum “dataset attach” machinery so Query Service can query Parquet safely:
- **trusted code** attaches datasets into DuckDB relations, then
- **untrusted SQL** may only query those relations (still gated by `trace-core::query::validate_sql`).

### Context links
- `docs/architecture/containers/query_service.md`
- `docs/specs/query_sql_gating.md`
- `docs/specs/query_service_task_query.md`
- `docs/adr/0008-dataset-registry-and-publishing.md`
- `docs/adr/0009-atomic-cutover-and-query-pinning.md`
- `docs/architecture/data_model/orchestration.md` (datasets + dataset_versions)

### Deliverables
- Extend dataset grants in the **task capability token** so Query Service can be deterministic/pinned:
  - include `dataset_uuid`
  - include `dataset_version_uuid` (pinned)
  - include a version-resolved storage reference (manifest object key and/or version-addressed prefix)
- Query Service “trusted attach” step:
  - resolves the granted dataset version to a fixed manifest/file list at query start
  - attaches Parquet objects as one or more DuckDB relations with safe deterministic names
  - executes gated SQL against those relations
- Negative tests proving fail-closed sandbox behavior:
  - `read_parquet(...)`, `parquet_scan(...)`, `FROM 'file'`, URL readers, `ATTACH`, `INSTALL/LOAD` remain blocked
  - querying an attached dataset relation succeeds

**Docs updates done as part of ms/11:**
- `docs/architecture/contracts.md`: clarify dataset grants in the capability token (pinned versions + storage refs) and how QS consumes them.
- `docs/architecture/data_model/orchestration.md`: tighten `dataset_versions.storage_location` semantics for Parquet datasets (manifest vs prefix, and pinning rules).
- `docs/architecture/containers/query_service.md`: document the attach strategy and the boundary: “trusted attach, untrusted SQL”.

### STOP gate
- `cd crates/trace-core && cargo test --offline`
- `cd crates/trace-query-service && cargo test -- --nocapture`
- `cd harness && cargo test -- --nocapture`

---

## Milestone 12: Cryo local sync worker (Lite)

Status: **complete** (tag: `ms/12`).

### Goal
Run Cryo locally to produce Parquet datasets and register dataset versions in Postgres state - no AWS required.

### Context links
- `docs/specs/operators/cryo_ingest.md`
- `docs/specs/ingestion.md`
- `docs/architecture/data_model/orchestration.md` (dataset_versions)

### Deliverables
- Implement a **trusted platform worker** path for `cryo_ingest` in Lite:
  - writes Parquet to MinIO using deterministic object keys (no Trace-owned manifest required)
  - uses a version-addressed prefix: `s3://{bucket}/cryo/{chain_id}/{dataset_uuid}/{range_start}_{range_end}/{dataset_version}/`
  - registers dataset versions in Postgres state (`state.dataset_versions`) with stable `{dataset_uuid, dataset_version, storage_prefix, config_hash, range_start, range_end}`
  - idempotent under retries: the same `{chain_id, range, config_hash}` must map to the same deterministic `dataset_version` (conflicts must match or fail)
- Explicitly **no** relational schema requirement for chain datasets in v1:
  - the Parquet schema is canonical (optionally record `schema_hash` from Parquet metadata)

### STOP gate
- Add a harness/integration test that runs the cryo worker twice for the same range and asserts:
  - a single dataset version (or deterministic ID), and
  - stable `storage_prefix` / manifest content

---

## Milestone 13: Lite chain sync planner (genesis → tip)

Status: **complete** (tag: `ms/13`).

Note: The ms/13 planner CLI (`trace-dispatcher plan-chain-sync`) is deprecated and removed; current deployments use `trace-dispatcher apply --file <spec.yaml>` and `trace-dispatcher status` (see ms/16).

### Goal
End-to-end local chain sync planning: schedule bounded Cryo ingestion ranges, enqueue tasks, and advance progress safely under restarts.

### Context links
- `docs/specs/chain_sync_entrypoint.md`
- `docs/specs/ingestion.md`
- `docs/architecture/task_lifecycle.md`
- `docs/specs/operators/cryo_ingest.md`
- `docs/architecture/data_versioning.md` (cursors + invalidations)

### Deliverables
- Planner entrypoint (historical, ms/13):
  - CLI: `trace-dispatcher plan-chain-sync --chain-id ... --from-block ... --to-block ... --chunk-size ... --max-inflight ...`
  - v1 simplicity: **no** RPC head lookup; the caller supplies an explicit `to_block` bound.
- State tables (Postgres state DB):
  - `state.chain_sync_cursor` stores the exclusive high-water mark `next_block`.
  - `state.chain_sync_scheduled_ranges` tracks each planned end-exclusive range (`[range_start, range_end)`) and its status (`scheduled` → `completed`).
- Idempotency + correctness under failure:
  - The planner is safe to re-run: it uses row locking on the cursor row and `ON CONFLICT DO NOTHING` on `(chain_id, range_start, range_end)` to avoid duplicates.
  - It never schedules more than `max_inflight` ranges ahead of completion.
- Parallelization model:
  - Each planned range creates one `cryo_ingest` task and enqueues a single wakeup message.
  - Any number of `cryo_ingest` workers may run concurrently; the queue provides fan-out.
- Completion linkage:
  - When a `cryo_ingest` task attempt completes successfully and publishes a dataset version, Dispatcher marks the corresponding scheduled range `completed`.

### STOP gate
- `cd crates/trace-dispatcher && cargo test -- --nocapture`
- `cd harness && cargo test -- --nocapture` (includes `planner_bootstrap_sync_schedules_and_completes_ranges`)

---

## Milestone 14: Alert evaluation over Parquet datasets

Status: **complete** (tag: `ms/14`).

### Goal
Prove the data path from “synced Parquet datasets” → “Query Service” → “alert event sink (idempotent)”.

### Context links
- `docs/specs/alerting.md`
- `docs/specs/query_service_task_query.md`
- `docs/specs/operators/alert_evaluate.md`
- `docs/adr/0004-alert-event-sinks.md`
- `docs/architecture/invariants.md`

### Deliverables
- Evaluation operator reads Parquet datasets through Query Service attached relations
- Produces alert events via buffered dataset → sink
- End-to-end invariant test:
  - dataset grant allows task query and emits audit
  - malformed outputs are rejected (fail closed)
  - retries do not duplicate alert events

### STOP gate
- `cd harness && cargo test -- --nocapture` includes a true E2E path (not fixture-only) over at least one attached Parquet dataset

---

## Milestone 15: Chain sync DAG entrypoint (spec locked; no code)

Status: **complete** (tag: `ms/15`).

### Goal
Lock a v1-safe, declarative `chain_sync` entrypoint that can sync multiple Cryo datasets from genesis to tip while preserving:
- dispatcher-owned planning (no external loops),
- leased tasks and at-least-once semantics,
- idempotent dataset publication and registry behavior, and
- Query Service fail-closed remote Parquet query safety.

### Context links
- `docs/specs/chain_sync_entrypoint.md`
- `docs/specs/ingestion.md`
- `docs/specs/dag_configuration.md`
- `docs/specs/operators/cryo_ingest.md`
- `docs/architecture/task_lifecycle.md`
- `docs/architecture/contracts.md`
- `docs/specs/query_service_task_query.md`
- `docs/specs/query_sql_gating.md`

### Deliverables
- A template-complete spec defining:
  - concepts/definitions (`dataset_key`, range partitions, per-stream cursors),
  - invariants and completion rules (`fixed_target` vs `follow_head`),
  - required state model and uniqueness constraints (semantic level only),
  - two candidate YAML shapes + a recommended v1 choice,
  - security constraints (dataset grants + QS remote scan egress allowlist).
- Milestone plan updated to include the spec freeze STOP boundary.

### STOP gate (mandatory)
Do not implement schema/code until the YAML shape and invariants in the spec are explicitly approved.

---

## Milestone 16: Chain sync job runner (multi-dataset)

Status: **complete** (tag: `ms/16`).

### Goal
Implement the `chain_sync` entrypoint described in ms/15 so Dispatcher can run “genesis → tip” sync internally for multiple Cryo datasets, with durable per-stream progress and bounded in-flight scheduling.

### Context links
- `docs/specs/chain_sync_entrypoint.md`
- `docs/specs/ingestion.md`
- `docs/specs/operators/cryo_ingest.md`
- `docs/architecture/task_lifecycle.md`
- `docs/architecture/contracts.md`
- `docs/specs/query_service_task_query.md`
- `docs/specs/query_sql_gating.md`
- `docs/examples/lite_local_cryo_sync.md`
- `docs/examples/chain_sync.monad_mainnet.yaml`

### Deliverables (high level)
- Persisted `chain_sync` job definitions via `trace-dispatcher apply --file <job.yaml>` and per-stream cursors.
  - Pause/resume is supported via `trace-dispatcher chain-sync pause|resume` which toggles `state.chain_sync_jobs.enabled`.
- Scheduled range ledger per dataset stream to guarantee idempotent planning.
- Dispatcher loop that tops up inflight work (no external loops), using the outbox + task queue wakeups.
- Per-task payload includes `{chain_id, dataset_key, dataset_uuid, range, rpc_pool}`.
- Each successful task completion publishes exactly one dataset version (single-publication rule).
- Harness/integration tests proving:
  - re-running planner is idempotent (no duplicate effective work).
  - retries do not double-register dataset versions.
  - Query Service remains fail-closed and can query the produced datasets via pinned grants.
  - fixed_target planning + Cryo invocation are end-exclusive: syncing [from_block, to_block) includes block (to_block - 1).

### STOP gate
- `cd harness && cargo test -- --nocapture`

---

## Milestone 17: `trace-lite` runnable local stack

Status: **complete** (tag: `ms/17`).

### Goal
Make “run it locally and sync a chain” a one-command experience.

### Deliverables
- Docker Compose profile (or equivalent) that runs:
  - Postgres (state), MinIO, dispatcher, sink, query-service, and cryo workers
- Minimal CLI wrappers:
  - `trace-lite up`
  - `trace-lite apply --file <path/to/job.yaml>`
    - YAML is the source of truth for the job graph; `trace-lite` must *not* re-invent planner flags.
  - `trace-lite status [--job <job_id>]`
    - Thin wrapper around `trace-dispatcher status [--job <job_id>]`.

Docs
- `docs/plan/trace_lite.md` (what `trace-lite` does / does not do)
- `docs/examples/lite_local_cryo_sync.md` (end-to-end runbook + smoke-test checklist)

### STOP gate
- Documented smoke test checklist with expected artifacts (datasets in MinIO + registry rows + query success)
  - See: `docs/examples/lite_local_cryo_sync.md`

---
