# Milestones

This file is the canonical ledger for:

- completed milestones (with immutable git tags), and
- planned milestones (next work).

See `AGENTS.md` in the repo root for milestone workflow rules (STOP gates, context links, harness commands).

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
| 12 | ms/12 | 339bef6 | Cryo ingest worker writes Parquet+manifest to MinIO; registers dataset_versions idempotently |
| 13 | ms/13 | 93e74da | Lite chain sync planner (cursor + scheduled ranges) + harness E2E |
| 14 | ms/14 | 005036d | Alert evaluation over Parquet datasets (QS -> UDF -> sink) + harness E2E |

### How to review a milestone

Given two milestone tags (example: ms/6 and ms/7):

    git diff --stat ms/6..ms/7
    git log --oneline ms/6..ms/7
    git checkout ms/7

Then run the milestone gates described in `AGENTS.md` (root of repo).

## Planned milestones (next)

Milestones **after ms/14** are sequenced to prove a full **Lite** deployment that can:

- run the platform services locally,
- sync a chain locally using **Cryo**,
- store chain datasets as **Parquet** in object storage (MinIO locally),
- query those datasets safely via Query Service **without** allowing untrusted SQL to read arbitrary files/URLs,
- and only *then* move to AWS deployment.

The table is the short index. Detailed deliverables + STOP gates follow.

| Milestone | Title | Notes |
|----------:|-------|-------|
| 15 | Chain sync DAG entrypoint (spec locked) | Spec-only. STOP: do not implement until YAML shape + invariants are approved. |
| 16 | Chain sync job runner (multi-dataset) | Implement dispatcher-owned planning for per-dataset cursors + scheduled ranges. |
| 17 | `trace-lite` runnable local stack | Docker Compose + runbook + minimal CLI wrappers to “bring up + sync” |
| 18 | Bundle manifest + real multi-language UDF runtime | Signed bundle manifests + hash/size checks; Node/Python first; Rust via common tooling |
| 19 | Minimal user API v1 | Bundle upload + DAG registration + publish datasets + alert definition CRUD |
| 20 | AWS deployable MVP | IaC + IAM/network boundaries + S3/SQS/Lambda wiring + smoke tests |
| S1 | Security gate: Query Service egress allowlist | Mandatory before any non-dev deployment that allows remote Parquet scans |

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
- `docs/architecture/operators/cryo_ingest.md`
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

### Goal
End-to-end local chain sync planning: schedule bounded Cryo ingestion ranges, enqueue tasks, and advance progress safely under restarts.

### Context links
- `docs/specs/lite_chain_sync_planner.md`
- `docs/specs/ingestion.md`
- `docs/architecture/task_lifecycle.md`
- `docs/architecture/operators/cryo_ingest.md`
- `docs/architecture/data_versioning.md` (cursors + invalidations)

### Deliverables
- Planner entrypoint (in `trace-dispatcher`):
  - CLI: `trace-dispatcher plan-chain-sync --chain-id ... --from-block ... --to-block ... --chunk-size ... --max-inflight ...`
  - v1 simplicity: **no** RPC head lookup; the caller supplies an explicit `to_block` bound.
- State tables (Postgres state DB):
  - `state.chain_sync_cursor` stores the exclusive high-water mark `next_block`.
  - `state.chain_sync_scheduled_ranges` tracks each planned inclusive range and its status (`scheduled` → `completed`).
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

## Milestone 15: Chain sync DAG entrypoint (spec locked; no code)

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
- `docs/architecture/operators/cryo_ingest.md`
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

### Goal
Implement the `chain_sync` entrypoint described in ms/15 so Dispatcher can run “genesis → tip” sync internally for multiple Cryo datasets, with durable per-stream progress and bounded in-flight scheduling.

### Context links
- `docs/specs/chain_sync_entrypoint.md`
- `docs/specs/ingestion.md`
- `docs/architecture/operators/cryo_ingest.md`
- `docs/architecture/task_lifecycle.md`
- `docs/architecture/contracts.md`
- `docs/specs/query_service_task_query.md`
- `docs/specs/query_sql_gating.md`
- `docs/deploy/lite_local_cryo_sync.md`
- `docs/examples/chain_sync.monad_mainnet.yaml`

### Deliverables (high level)
- Persisted `chain_sync` job definitions via `trace-dispatcher apply --file <job.yaml>` and per-stream cursors.
  - Pausing is done by re-applying the YAML with `enabled: false` (no separate pause/resume subcommands in v1).
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

### Goal
Make “run it locally and sync a chain” a one-command experience.

### Deliverables
- Docker Compose profile (or equivalent) that runs:
  - Postgres (state), MinIO, dispatcher, sink, query-service, and cryo workers
- Minimal CLI wrappers:
  - `trace-lite up`
  - `trace-lite apply --file <path/to/job.yaml>`
    - YAML is the source of truth for the job graph; `trace-lite` must *not* re-invent planner flags.
  - `trace-lite status --job <job_id>`
    - Thin wrapper around `trace-dispatcher status --job <job_id>`.

### STOP gate
- Documented smoke test checklist with expected artifacts (datasets in MinIO + registry rows + query success)

---

## Milestone 18: Bundle manifest + real multi-language UDF runtime

Goal: replace harness-only runner logic with a real bundle model that is safe under retries and supports multiple languages.

Notes:
- Signed manifests, hash/size checks, and fail-closed fetch/execution rules.
- Node/Python first; Rust follows common packaging/tooling.

---

## Milestone 19: Minimal user API v1

Goal: expose only the smallest stable public surface (everything else remains internal).

Deliverables:
- Bundle upload + DAG registration
- Publish datasets (make chain sync datasets queryable by users)
- `POST /v1/query` - user-facing interactive query endpoint (Query Service)
  - Requires: dataset registry lookup, user/org authz (JWT), result persistence for large results
  - Blocked by: dataset publishing mechanism from chain sync
- Alert definition CRUD

---

## Milestone 20: AWS deployable MVP

Goal: move the proven Lite semantics to AWS adapters + deployable infra (S3/SQS/Lambda/IAM/VPC).
---

## Security Gate S1: Query Service egress allowlist

### Why this exists
If Query Service is allowed to scan *authorized* remote Parquet datasets (HTTP/S3) during query execution, DuckDB becomes a **network-capable** process.

If the SQL gate is ever bypassed (bug, misconfiguration, future feature), an attacker could try to use Query Service as an SSRF / exfiltration primitive.

This gate makes the trust boundary enforceable by requiring **OS/container-level egress allowlists**.

### Scope
This gate applies to **Query Service** only.

(It does **not** replace the existing “no third-party internet egress” requirement for untrusted UDF runtimes; that requirement remains and is tracked elsewhere.)

### Context links
- `docs/specs/query_sql_gating.md` (notes on remote Parquet + sandbox)
- `docs/specs/query_service_task_query.md` (task query threat model)
- `docs/architecture/containers/query_service.md` (DuckDB hardening + attach strategy)
- `docs/adr/0002-networking.md` (no egress by default + egress services)
- `docs/standards/security_hardening.md` (egress allowlists)

### Deliverables
- **Lite/local** (ms/15): document the local posture explicitly:
  - By default, Lite/dev may not enforce strict egress controls.
  - If remote Parquet scans are enabled, provide a recommended enforcement approach (container network policy / host firewall) and a verification checklist.
- **AWS** (ms/18): enforce a strict egress allowlist:
  - Query Service must run in private subnets with **no general NAT egress**.
  - Allow egress only to:
    - the configured object store endpoint(s) (S3 via VPC endpoint), and
    - internal platform services as required.
  - “Only Delivery Service and RPC Egress Gateway have outbound internet egress” remains true.

### Verification
- From inside the Query Service container/task:
  - Object store endpoint is reachable.
  - An arbitrary public endpoint is **not** reachable (fail closed).
- Keep the SQL gate tests green (this gate is defense-in-depth, not a replacement).
