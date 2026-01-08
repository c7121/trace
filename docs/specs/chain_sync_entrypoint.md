# Chain Sync DAG Entrypoint (genesis → tip, multi Cryo datasets)

Status: Draft
Owner: Platform
Last updated: 2026-01-08

Keep this document short. Delete sections that do not apply.

## Summary
Define a declarative `chain_sync` DAG entrypoint that syncs one or more Cryo datasets (e.g., `blocks`, `logs`, `geth_calls`) from genesis to tip. The Dispatcher owns all planning/scheduling (no shell loops) using durable Postgres state, leased tasks, and idempotent dataset publication so outputs remain safely queryable via Query Service remote Parquet scans.

## Risk
High

High risk includes new public surface, data migrations, auth/authz changes, trust-boundary changes,
new persistence invariants, significant perf/latency impact.

If Risk is High:
- Monitoring signals and rollback MUST be explicit.
- Ask before proceeding from spec to implementation.

## Related ADRs
- ADR-0002 (Networking / egress allowlist)
- ADR-0008 (Dataset registry and publishing)
- ADR-0009 (Atomic cutover and query pinning)

## Context
Today, Lite bootstrap sync is proven via:
- `cryo_ingest` producing Parquet + a Trace-owned manifest under a deterministic version-addressed prefix (ms/12),
- `dataset_versions` registered in Postgres state idempotently on task completion, and
- a planner CLI (`plan-chain-sync`, ms/13) that schedules bounded ranges but requires an external caller to re-run it and only plans a single dataset stream per invocation.

Problem statement:
Users want to specify “sync this chain” once and have the system run to completion (or continuously follow head) across multiple Cryo datasets, without external planning loops. The outputs must remain queryable via Query Service while preserving the existing sandbox and security invariants (capability tokens, dataset grants, remote Parquet scan, fail-closed behavior).

Constraints that matter:
- Task execution is at-least-once; duplicates and restarts are expected (see `docs/architecture/task_lifecycle.md`).
- Dispatcher owns scheduling; workers must not decide retries/scheduling.
- Query Service executes untrusted SQL and MUST stay fail-closed behind `trace_core::query::validate_sql` + runtime hardening (see `docs/specs/query_sql_gating.md` and `docs/specs/query_service_task_query.md`).
- Chain datasets have no required relational schema in v1; Parquet is canonical (see `docs/specs/ingestion.md`).

## Goals
- Provide a declarative, reviewable configuration for “sync chain from genesis → tip” that can include multiple Cryo datasets in one job.
- Make planning internal: Dispatcher continuously “tops up” in-flight work up to a configured bound (no external loop).
- Persist progress durably and monotonically via per-(chain_id, dataset_key) cursors.
- Ensure idempotency:
  - re-planning does not create duplicate effective work for the same range, and
  - task retries do not double-register dataset versions.
- Support dataset-specific RPC pools (e.g., `geth_calls` uses trace-enabled providers).

## Non-goals
- Reorg-safe “follow head” semantics beyond a minimal head observation and bounded tail window (full reorg handling is a separate operator concern).
- Cross-dataset joins or schema enforcement at ingestion time.
- Public user APIs for starting/stopping sync (this spec defines an admin surface only).
- Exactly-once scheduling or exactly-once ingestion.

## Public surface changes
Required. List any new/changed public surfaces. If none, write “None”.

Public surface includes endpoints/RPC, event schemas/topics, CLI flags/commands, config semantics,
persistence formats/migrations, and entrypoint exports.

- Endpoints/RPC:
  - Admin-only: apply/pause/resume/status for `chain_sync` jobs (shape TBD; see “API surface”).
- Events/schemas:
  - Task payload schema for `cryo_ingest` tasks planned by `chain_sync` MUST include:
    - `chain_id` (int)
    - `dataset_key` (string)
    - `dataset_uuid` (uuid)
    - `cryo_dataset_name` (string)
    - `rpc_pool` (string)
    - `range_start` (int)
    - `range_end` (int, end-exclusive in payload terms)
    - `config_hash` (string)
- CLI:
  - Admin-only: `trace-dispatcher chain-sync apply|pause|resume|status` (names TBD).
- Config semantics:
  - DAG YAML gains a `chain_sync` entrypoint (two candidate shapes in this spec; one will be selected for v1).
- Persistence format/migration:
  - Postgres *state* requires durable `chain_sync` job definitions + per-stream cursors + scheduled range ledger (semantic model in this spec; SQL not designed here).
- Entrypoint exports:
  - None.
- Intentionally not supported (surface area control):
  - External shell loop planners.
  - User SQL referencing any Parquet paths/URLs directly (Query Service continues to attach relations in trusted code).

## Architecture (C4) - Mermaid-in-Markdown only
Use the smallest C4 level(s) that clarify the change (L1/L2/L3).
No box soup: every box MUST have a responsibility; every relationship MUST be labeled.

```mermaid
flowchart LR
  ADMIN[Admin/Operator] -->|apply chain_sync config| DISP[Dispatcher]
  DISP -->|persist jobs/cursors/ranges| STATE[(Postgres state)]
  DISP -->|enqueue task wakeups| Q[Task Queue]
  Q -->|task_id wakeups| CW[Cryo workers]
  CW -->|write Parquet+manifest| OBJ[(Object store)]
  CW -->|/v1/task/complete + dataset_publication| DISP
  DISP -->|register dataset_versions| STATE
  QS[Query Service] -->|fetch manifest + scan Parquet via httpfs| OBJ
  QS -->|audit| DATA[(Postgres data)]
```

## Proposed design
Bullets. Include only what is needed to implement/review safely.

### Concepts / definitions (enforceable)
- Entrypoint: `chain_sync`.
- Dataset stream: `{ dataset_key, chain_id }`.
  - `dataset_key` is a Cryo dataset identifier (e.g., `blocks`, `logs`, `geth_calls`).
  - Each dataset stream maps to exactly one published dataset UUID in the dataset registry (resolution is part of “apply”).
- Partition/range: `[start, end)` block interval (end-exclusive).
- Task: one `{ dataset_key, chain_id, dataset_uuid, range, rpc_pool }` planned unit of work.
  - `rpc_pool` selects an RPC pool owned by the RPC Egress Gateway (API keys are not in YAML).
  - `dataset_uuid` is inserted into the task payload by the Dispatcher at planning time after apply-time resolution.
  - **Single-output rule (v1):** a task attempt is defined to produce **exactly one** `DatasetPublication`.
    - A completion payload that contains zero publications or attempts to publish more than one publication is malformed and MUST be rejected by the Dispatcher.
    - Multi-output tasks (e.g., “one task runs Cryo for blocks+logs+traces in one invocation”) are intentionally not supported in v1 because they couple failure domains and break retry/idempotency invariants.
- DatasetPublication: on successful task completion, a single dataset version publication describing:
  - `{ dataset_uuid, dataset_version, storage_ref, config_hash, range_start, range_end }`.
- Cursor: per `{ chain_id, dataset_key }` exclusive high-water mark `next_block` for planning.
- Tip mode:
  - `fixed_target`: plan from `from_block` until `cursor >= to_block`, then complete.
  - `follow_head`: continuously plan as head advances; does not “complete”.

### Semantics / invariants (non-negotiable)
- No external planning loops: Dispatcher continuously tops up in-flight work for each active `chain_sync` job.
- Multiple datasets per job are supported: a single `chain_sync` job definition contains N dataset streams.
- **Each successful task completion MUST include exactly one dataset publication.**
  - The Dispatcher MUST reject completion if the publication is missing or if the completion attempts multiple publications.
- Different dataset streams MAY use different `rpc_pool` values.
- Idempotency:
  - Re-planning MUST NOT create duplicate scheduled ranges for the same `{job_id, dataset_key, range}` (enforced by a uniqueness constraint).
  - Task retries MUST NOT double-register dataset versions (enforced by deterministic `dataset_version` and/or registry uniqueness; conflicts must match or fail).
- Completion:
  - `fixed_target`: a job is complete iff for every dataset stream: `cursor >= to_block` AND there are zero in-flight scheduled ranges.
  - `follow_head`: the job never completes; it maintains an in-flight window relative to observed head.
- Queryability:
  - Every dataset publication MUST resolve to a `storage_ref` that Query Service can attach remotely (no Parquet downloads by Query Service).
  - Untrusted SQL remains gated by `trace_core::query::validate_sql` and MUST NOT mention file/URL literals, `read_parquet/parquet_scan`, `ATTACH`, `INSTALL/LOAD`, or multi-statements.
- Security:
  - Dataset grants in the task capability token MUST bound dataset access (dataset UUID/version + storage ref).
  - Query Service remote Parquet scans require an egress allowlist (only object-store endpoints allowed) and MUST fail closed if misconfigured.

### Planner algorithm (internal, restart-safe)
For each active `chain_sync` job, Dispatcher runs a loop (periodic, e.g. every few seconds):
1) Load the job definition and its dataset stream configs.
2) For each dataset stream, compute an eligible planning window:
   - `fixed_target`: `to_block` is fixed.
   - `follow_head`:
     - If head is missing or stale, planning MUST skip for that stream (fail closed) rather than guessing.
     - Use end-exclusive math:
       - `to_block = max(from_block, (observed_head + 1) - tail_lag)`
3) While `inflight_count(stream) < max_inflight(stream)` and `cursor(stream) < to_block`:
   - compute next `[start,end)` by `chunk_size`.
   - atomically insert a scheduled-range row keyed by `{job_id, dataset_key, range_start, range_end}`.
     - If it already exists, skip (idempotency).
   - atomically create (or fetch) a task row keyed by the same range (task dedupe key).
   - write an outbox wakeup for that task.
   - advance the cursor to `end` in the same transaction as recording the scheduled range + enqueue outbox.

### Completion semantics for scheduled ranges
- A scheduled range moves `scheduled → completed` only when the corresponding task attempt completes successfully and its dataset publication is accepted (attempt-fenced).
- Failed/expired attempts do not advance completion; retries reuse the same task id and scheduled range record.

### State model (semantic; not SQL)
The following durable state is required in Postgres *state* (naming is illustrative):
- Sync job definition (`chain_sync_jobs`):
  - Identity:
    - `job_id` (stable UUID)
    - `(org_id, name)` unique key (apply idempotency)
  - Core fields:
    - `org_id` (uuid)
    - `name` (string)
    - `chain_id` (int)
    - `enabled` (bool)
    - `mode` (`fixed_target|follow_head`)
    - `from_block` (int)
    - `to_block` (int, nullable for follow head)
    - defaults: `chunk_size`, `max_inflight`
  - Follow-head fields:
    - `tail_lag` (int blocks)
    - `head_poll_interval_seconds` (int)
    - `max_head_age_seconds` (int)
  - Audit/debug:
    - `yaml_hash` (string; change detector only)
    - `updated_at`
    - `last_error` (optional category, redacted message)
- Stream definitions (`chain_sync_streams`):
  - key: `{job_id, dataset_key}`
  - fields:
    - `dataset_key` (string)
    - `cryo_dataset_name` (string)
    - `rpc_pool` (string)
    - derived identity: `dataset_uuid` (uuid, deterministic from `{org_id, chain_id, dataset_key}`)
    - `config_hash` (string)
    - per-stream overrides: `chunk_size`, `max_inflight`
- Per-stream cursor (`chain_sync_cursor`):
  - key: `{job_id, dataset_key}`
  - fields:
    - `next_block` (exclusive high-water mark)
    - `updated_at`
- Scheduled range ledger (`chain_sync_scheduled_ranges`):
  - key: `{job_id, dataset_key, range_start, range_end}`
  - fields:
    - `task_id` (stable mapping to the planned task)
    - `status` (`scheduled|completed`)
    - `created_at`, `updated_at`
  - uniqueness: `(job_id, dataset_key, range_start, range_end)` MUST be unique
- Optional head observation (`chain_head_observations`):
  - key: `{chain_id}`
  - `head_block` and `observed_at`
  - `source` (e.g., rpc_pool) for debugging only

### API surface (admin-only; proposal)
This spec defines the minimum required operations; concrete endpoint/CLI names are not final.

Operations:
- Apply spec (YAML): creates/updates a `chain_sync` job definition.
  - Apply MUST be idempotent by `(org_id, job name)` (or an explicit `job_id`).
  - `yaml_hash` is stored as a change detector and audit value, not the identity.
- Pause/resume: flips job status; planner loop must stop scheduling new work when paused.
- Status/progress:
  - per dataset stream cursor (`next_block`)
  - inflight scheduled count
  - last error category + timestamp (no secrets)
  - head observed timestamp (if follow head mode)

### DAG YAML (candidate shapes)
This section proposes two YAML shapes to express a `chain_sync` entrypoint. One MUST be selected and locked before implementation.

Constraints (v1):
- YAML MUST NOT contain secrets, including RPC URLs, API keys, or object store credentials.
- YAML MUST reference RPC pools by name only (`rpc_pool: traces`), never by URL.
- Each dataset stream becomes its own planned task stream. Tasks are per `{dataset_key, range}`.
- The single-output rule remains: one successful task completion yields exactly one dataset publication.
- Connections are deferred unless explicitly selected for v1 (see Open decisions).

#### Option A: DAG-native entrypoint
`chain_sync` is a first-class entrypoint inside the DAG YAML.

Minimal schema:
- `entrypoints[]` list contains:
  - `name` unique within the DAG
  - `kind: chain_sync`
  - `chain_id`
  - `mode`:
    - `fixed_target`: `{ from_block, to_block }` where `to_block` is end-exclusive
      - Note: to sync through block `N` inclusive, set `to_block = N + 1`.
    - `follow_head`: `{ from_block, tail_lag, head_poll_interval_seconds, max_head_age_seconds }`
  - `streams[]` list of dataset streams:
    - `dataset_key` string (Cryo dataset identifier)
    - `cryo_dataset_name` string (Cryo CLI dataset name, v1: same as `dataset_key`)
    - `rpc_pool` string (name only)
    - `chunk_size` block count
    - `max_inflight` planned range count cap

Concrete example (bootstrap, end-exclusive):
```yaml
name: mainnet_bootstrap

entrypoints:
  - name: sync_mainnet
    kind: chain_sync
    chain_id: 1
    mode:
      kind: fixed_target
      from_block: 0
      to_block: 20000000 # syncs [0, 20000000)
    streams:
      - dataset_key: blocks
        cryo_dataset_name: blocks
        rpc_pool: standard
        chunk_size: 2000
        max_inflight: 40
      - dataset_key: logs
        cryo_dataset_name: logs
        rpc_pool: standard
        chunk_size: 1000
        max_inflight: 20
      - dataset_key: geth_logs
        cryo_dataset_name: geth_logs
        rpc_pool: standard
        chunk_size: 1000
        max_inflight: 10
      - dataset_key: geth_calls
        cryo_dataset_name: geth_calls
        rpc_pool: traces
        chunk_size: 500
        max_inflight: 5
```

Pros:
- Single DAG document remains the unit of review and deployment.
- Keeps future composition explicit: `chain_sync` streams can be wired to downstream jobs without introducing an out-of-band compiler.
- Reduces drift risk between the DAG config spec and the chain sync config.

Cons:
- Expands DAG YAML surface area and validator complexity.
- Requires stable stream referencing rules to avoid ambiguous connections.

#### Option B: Shorthand job YAML
`chain_sync` is defined in its own document that compiles into Option A.

Minimal schema:
- Top-level document:
  - `kind: chain_sync`
  - `name`
  - `chain_id`
  - `mode`:
    - `fixed_target`: `{ from_block, to_block }` where `to_block` is end-exclusive
      - Note: to sync through block `N` inclusive, set `to_block = N + 1`.
    - `follow_head`: `{ from_block, tail_lag, head_poll_interval_seconds, max_head_age_seconds }`
  - `streams` mapping from `dataset_key` to:
    - `cryo_dataset_name` (string)
    - `rpc_pool` name only
    - `chunk_size`
    - `max_inflight`

Concrete example (bootstrap, end-exclusive):
```yaml
kind: chain_sync
name: mainnet_bootstrap
chain_id: 1

mode:
  kind: fixed_target
  from_block: 0
  to_block: 20000000 # syncs [0, 20000000)

streams:
  blocks:
    cryo_dataset_name: blocks
    rpc_pool: standard
    chunk_size: 2000
    max_inflight: 40
  logs:
    cryo_dataset_name: logs
    rpc_pool: standard
    chunk_size: 1000
    max_inflight: 20
  geth_logs:
    cryo_dataset_name: geth_logs
    rpc_pool: standard
    chunk_size: 1000
    max_inflight: 10
  geth_calls:
    cryo_dataset_name: geth_calls
    rpc_pool: traces
    chunk_size: 500
    max_inflight: 5
```

Concrete example (follow-head):
```yaml
kind: chain_sync
name: mainnet_follow
chain_id: 1

mode:
  kind: follow_head
  from_block: 0
  tail_lag: 64
  head_poll_interval_seconds: 5
  max_head_age_seconds: 30

streams:
  blocks:
    cryo_dataset_name: blocks
    rpc_pool: standard
    chunk_size: 2000
    max_inflight: 40
  logs:
    cryo_dataset_name: logs
    rpc_pool: standard
    chunk_size: 1000
    max_inflight: 20
  geth_logs:
    cryo_dataset_name: geth_logs
    rpc_pool: standard
    chunk_size: 1000
    max_inflight: 10
  geth_calls:
    cryo_dataset_name: geth_calls
    rpc_pool: traces
    chunk_size: 500
    max_inflight: 5
```

Pros:
- Minimizes the amount of DAG scaffolding required for the common "sync a chain" use case.
- Easier to present as an admin-only workflow without coupling to full DAG evolution.

Cons:
- Introduces a compiler/translation step that can drift from the canonical DAG spec.
- Future DAG composition becomes harder to review because the compiled DAG is the true executed artifact.

Open decisions:
- Select Option A or Option B for v1. Current working choice: Option B.
- Connections are deferred in v1 unless explicitly selected.
- `dataset_uuid` resolution:
  - v1 MUST use a deterministic mapping from `{org_id, chain_id, dataset_key}` rather than user-supplied UUIDs in YAML.

## Contract requirements
Use MUST/SHOULD/MAY only for behavioral/contract requirements (not narrative).
- The Dispatcher MUST be the only component that performs planning/scheduling for `chain_sync` (no external loops).
- The Dispatcher MUST persist progress per `{chain_id, dataset_key}` as a monotonic cursor (`next_block`) and MUST NOT move it backwards.
- The planner MUST be idempotent under restart: inserting scheduled ranges MUST be protected by a uniqueness constraint and safe retries (`ON CONFLICT DO NOTHING` or equivalent).
- Each planned range MUST map to a stable task identity (dedupe key) so duplicate scheduling does not create divergent work.
- Workers MUST NOT register dataset versions directly; they MUST publish dataset publications only via fenced `/v1/task/complete`.
- For `cryo_ingest` tasks, `/v1/task/complete` MUST carry **exactly one** dataset publication object and it MUST match the task payload `{chain_id, dataset_key, dataset_uuid, range_start, range_end}`.
  - The completion's `dataset_publication.dataset_uuid` MUST equal the payload's `dataset_uuid`.
  - The Dispatcher MUST reject completions that omit the publication or attempt to include multiple publications (even if identical).
  - The Dispatcher MUST NOT partially accept a subset of publications.
- Dataset version registration MUST be idempotent: if a dataset version insert conflicts, the stored `{storage_ref, config_hash, range}` MUST match exactly or the completion MUST fail (no silent divergence).
- Query Service MUST remain fail-closed: it MUST attach datasets in trusted code and execute only validated SQL; it MUST NOT allow untrusted SQL to read arbitrary files/URLs.
- If Query Service remote Parquet scans are enabled, the deployment MUST enforce an egress allowlist permitting only object-store endpoints; otherwise Query Service MUST fail closed.

## Compatibility and migrations
- This entrypoint is intended to subsume the ms/13 `plan-chain-sync` CLI usage for new deployments.
- Existing Lite tests that rely on `plan-chain-sync` remain valid until the entrypoint is implemented and adopted.

## Security considerations
- Threats:
  - Untrusted SQL exfiltrates data via file/URL readers once remote Parquet scans exist.
  - Overbroad dataset grants allow a task to query or scan unrelated storage prefixes.
  - Planner drift causes duplicate/overlapping ranges that produce inconsistent dataset version registry state.
- Mitigations:
  - Keep Query Service validation as the single source of truth (`trace_core::query::validate_sql`) plus DuckDB runtime hardening.
  - Remote Parquet scans make Query Service a network-capable process; the threat model assumes deployment egress is limited to object store endpoints (see ADR-0002).
  - Capability tokens must pin dataset UUID/version and bound storage access via S3 read prefixes; Query Service must enforce both.
  - Scheduled-range uniqueness + deterministic dataset versioning at the registry boundary; reject divergence on conflict.
- Residual risk:
  - Validator denylist incompleteness; mitigated by defense-in-depth and tests.

## High risk addendum
### Observability and operability
- Monitoring signals:
  - Per stream: `cursor.next_block` vs `to_block` (fixed) or vs observed head (follow)
  - Inflight count and scheduled backlog per stream
  - Task failure rate by error category (RPC/Cryo/object-store)
  - Query Service denials (validate_sql rejects) and attach failures
- Logs:
  - MUST NOT log raw SQL in Query Service paths.
  - MUST redact secrets (RPC keys, object-store secrets); errors are categorized.

### Rollout and rollback
- Rollout:
  - Introduce entrypoint in DAG YAML behind admin-only apply; do not route via Gateway.
  - Implement fixed-target first; follow-head mode is feature-complete only when head observation is implemented.
- Rollback strategy:
  - Pause the job (stop scheduling) without deleting state.
  - If required, fall back to manual `plan-chain-sync` (ms/13) for bounded ranges.

## Reduction pass
Required.
How does this reduce or avoid expanding surface area?
- Avoided options/modes:
  - No external planning loops; no bespoke per-user scripts.
  - No per-job IAM policy modeling in YAML; RPC pools are referenced, not embedded.
- Consolidated paths:
  - Reuses existing leased task lifecycle, outbox, and dataset version publication instead of inventing a new scheduler.
- Simplified invariants:
  - Single cursor + scheduled range ledger per stream; deterministic dataset version boundary.

## Alternatives considered
Brief.
- Alternative: keep planning as an external CLI loop (`plan-chain-sync`) and document runbooks.
  - Why not: it is operationally fragile and violates “system manages planning internally”.
- Alternative: fully express chain sync as a generic DAG with explicit `range_splitter` nodes and per-dataset ingestion nodes.
  - Why not: heavier YAML surface for the common bootstrap case; harder to keep stable for v1.

## Acceptance criteria
Required.
- Tests (map to goals; include negative cases where relevant):
  - Planner idempotency: applying the same `chain_sync` spec twice does not duplicate scheduled ranges or tasks.
  - At-least-once safety: retrying a failed range does not double-register a dataset version.
  - Single-output enforcement: a task completion that omits `dataset_publication` or attempts to publish more than one publication is rejected and MUST NOT register any dataset version.
  - UUID binding: a task completion with a mismatched `dataset_uuid` is rejected and MUST NOT register any dataset version.
  - Security: Query Service continues to reject unsafe SQL and does not allow file/URL reads from untrusted SQL.
- Observable behavior:
  - Status reports per-stream cursor and inflight counts; fixed-target completion is computed precisely.
- Performance/SLO constraints (if any):
  - Planner loop must not hot-loop; bounded periodic scheduling is sufficient for v1.
