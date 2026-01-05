# Milestones

This file is the canonical ledger for:

- completed milestones (with immutable git tags), and
- planned milestones (next work).

Milestones are defined in more detail (gates, STOP points, scope) in `docs/plan/plan.md`.

## Completed milestones

Each completed milestone is pinned by an annotated git tag `ms/<N>` pointing at the STOP boundary commit.

| Milestone | Tag  | Commit    | Summary |
|----------:|------|-----------|---------|
| 1 | ms/1 | 31c6675 | Harness invariants + token overlap tests |
| 2 | ms/2 | d6b2d2d | Core traits + lite rewiring + aws adapters |
| 3 | ms/3 | 0f8cc26 | Runner payload + invoker/runner E2E + dispatcher client |
| 4 | ms/4 | 9beeb95 | SQL validation gate hardened |
| 5 | ms/5 | 22c3511 | Task query service + audit + deterministic DuckDB fixture |
| 6 | ms/6 | 9578de7 | Dataset grants enforced + docs corrected |
| 7 | ms/7 | 88dbd37 | Harness E2E invariant: dataset grant -> task query -> audit |
| 8 | ms/8 | 2334ae5 | Dispatcher extracted into `crates/trace-dispatcher` (harness wrapper kept) |
| 9 | ms/9 | 3892487 | Sink extracted into `crates/trace-sink` (harness wrapper kept) |
| 10 | ms/10 | 07a08df | RuntimeInvoker interface (lite + AWS Lambda feature-gated); harness routes UDF invocation via invoker |
| 11 | ms/11 | 319df13 | Parquet dataset versions pinned in task capability tokens; Query Service attaches via trusted manifest |

### How to review a milestone

Given two milestone tags (example: ms/6 and ms/7):

    git diff --stat ms/6..ms/7
    git log --oneline ms/6..ms/7
    git checkout ms/7

Then run the milestone gates described in `docs/plan/plan.md`.

## Planned milestones (next)

Milestones **after ms/11** are sequenced to prove a full **Lite** deployment that can:

- run the platform services locally,
- sync a chain locally using **Cryo**,
- store chain datasets as **Parquet** in object storage (MinIO locally),
- query those datasets safely via Query Service **without** allowing untrusted SQL to read arbitrary files/URLs,
- and only *then* move to AWS deployment.

The table is the short index. Detailed deliverables + STOP gates follow.

| Milestone | Title | Notes |
|----------:|-------|-------|
| 12 | Cryo local sync worker (Lite) | Run `cryo_ingest` locally to write Parquet+manifest to MinIO; register `dataset_versions` |
| 13 | Lite chain sync planner (genesis → tip) | Generate range tasks, track cursor/progress in Postgres state, parallelize via queue + rpc_pool |
| 14 | Alert evaluation over Parquet datasets | Evaluation reads Parquet via QS attached relations; emits buffered alert events; E2E invariant |
| 15 | `trace-lite` runnable local stack | Docker Compose + runbook + minimal CLI wrappers to “bring up + sync” |
| 16 | Bundle manifest + real multi-language UDF runtime | Signed bundle manifests + hash/size checks; Node/Python first; Rust via common tooling |
| 17 | Minimal user API v1 | Bundle upload + DAG registration + publish datasets + alert definition CRUD |
| 18 | AWS deployable MVP | IaC + IAM/network boundaries + S3/SQS/Lambda wiring + smoke tests |

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

### Goal
Run Cryo locally to produce Parquet datasets and register dataset versions in Postgres state — no AWS required.

### Context links
- `docs/architecture/operators/cryo_ingest.md`
- `docs/specs/ingestion.md`
- `docs/architecture/data_model/orchestration.md` (dataset_versions)

### Deliverables
- Implement a **trusted platform worker** path for `cryo_ingest` in Lite:
  - writes Parquet (and a manifest) to MinIO using deterministic object keys
  - registers/updates `dataset_versions.storage_location` to point at the manifest/prefix
  - idempotent under retries (same `{chain_id, range, config_hash}` does not double-register)
- Explicitly **no** relational schema requirement for chain datasets in v1:
  - the Parquet schema is canonical (optionally record `schema_hash` from Parquet metadata)

### STOP gate
- Add a harness/integration test that runs the cryo worker twice for the same range and asserts:
  - a single dataset version (or deterministic ID), and
  - stable `storage_location` / manifest content

---

## Milestone 13: Lite chain sync planner (genesis → tip)

### Goal
End-to-end local chain sync: plan ranges, enqueue tasks, parallelize work, advance progress cursor.

### Context links
- `docs/specs/ingestion.md`
- `docs/architecture/task_lifecycle.md`
- `docs/architecture/operators/range_splitter.md`
- `docs/architecture/operators/cryo_ingest.md`
- `docs/architecture/data_versioning.md` (cursors + invalidations)

### Deliverables
- Planner loop (v1 simplicity: may live in the dispatcher binary):
  - reads head block (RPC)
  - reads cursor/progress from Postgres state
  - emits bounded range tasks (`chunk_size`)
  - parallelizes via queue + multiple workers (capped by `scaling.max_concurrency`)
- RPC throughput strategy:
  - use `rpc_pool` selection; keys remain owned by RPC egress gateway config (never in DAG YAML)

### STOP gate
- Add a local runbook section: “bootstrap sync from block A to B with N workers”
- Add a crash/retry test proving progress advances without duplication

---

## Milestone 14: Alert evaluation over Parquet datasets

### Goal
Prove the data path from “synced Parquet datasets” → “Query Service” → “alert event sink (idempotent)”.

### Context links
- `docs/specs/query_service_task_query.md`
- `docs/architecture/operators/alert_evaluate.md`
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

## Milestone 15: `trace-lite` runnable local stack

### Goal
Make “run it locally and sync a chain” a one-command experience.

### Deliverables
- Docker Compose profile (or equivalent) that runs:
  - Postgres (state), MinIO, dispatcher, sink, query-service, and cryo workers
- Minimal CLI wrappers:
  - `trace-lite up`
  - `trace-lite sync --chain-id ... --from ... --to ... --chunk-size ... --workers N`
  - `trace-lite status`

### STOP gate
- Documented smoke test checklist with expected artifacts (datasets in MinIO + registry rows + query success)

---

## Milestone 16: Bundle manifest + real multi-language UDF runtime

Goal: replace harness-only runner logic with a real bundle model that is safe under retries and supports multiple languages.

Notes:
- Signed manifests, hash/size checks, and fail-closed fetch/execution rules.
- Node/Python first; Rust follows common packaging/tooling.

---

## Milestone 17: Minimal user API v1

Goal: expose only the smallest stable public surface (everything else remains internal).

---

## Milestone 18: AWS deployable MVP

Goal: move the proven Lite semantics to AWS adapters + deployable infra (S3/SQS/Lambda/IAM/VPC).
