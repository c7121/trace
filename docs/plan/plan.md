# Plan

This directory is sequencing guidance. It is intentionally **not normative**; the source of truth for behavior and invariants is:

- `docs/architecture/*` (C4, contracts, lifecycle)
- `docs/specs/*` (public surfaces)
- `docs/adr/*` (decisions)
- `docs/standards/*` (security + operations invariants)

## Next steps (do these first)

### 1) Contract-freeze “happy path” harness (MUST)

Implementation lives in `harness/`.
See `harness/README.md` and `harness/AGENT_TASKS.md`.


Goal: prove the core contracts are implementable and correct under at-least-once delivery **before** you build feature code.

**Recommendation:** implement this first against the **Trace Lite** profile (Postgres + MinIO + pgqueue). It gives you a deterministic harness for retries/duplicates without AWS integration churn. Once the harness passes, swap the adapters for AWS (SQS/S3/RDS).

Deliverables:
- A stub Dispatcher implementing only:
  - `/internal/task-claim` (lease acquisition; worker-only)
  - `/v1/task/heartbeat` (attempt-fenced)
  - `/v1/task/events` (attempt-fenced)
  - `/v1/task/buffer-publish` (attempt-fenced; pointer pattern)
  - `/v1/task/complete` (attempt-fenced; commit + route side effects via outbox)
- A stub worker that:
  - consumes a `task_id` wake-up message,
  - claims a lease,
  - emits one event,
  - publishes one buffer batch pointer,
  - completes.

**Test cases (minimum):**
- Duplicate wake-up messages do not cause concurrent execution (lease fencing).
- Stale attempt cannot heartbeat/emit/publish/complete (lease_token mismatch).
- Kill/restart Dispatcher mid-flight: outbox rows resume and side effects are not lost.
- Kill worker mid-task: lease expires, task retries, and only one attempt commits.
- Poison buffer batch artifact: sink rejects; message reaches DLQ; replay path is documented.

Exit criteria:
- You can repeatedly crash/restart components and still end with exactly one accepted completion per `(task_id, attempt)` (idempotent updates, no double-commit).
- All “duplicate” paths are safe no-ops (no manual SQL required).

### 2) Freeze the small-but-critical public surfaces

Lock these decisions before you implement operators:

- **Task auth model:** untrusted runtimes use **only** per-attempt capability tokens (+ lease fencing). No hidden shared secrets for Lambdas.
- **Capability token contract:** `X-Trace-Task-Capability` header + the token claim schema in `docs/architecture/contracts.md` are **normative**. Do not implement ad-hoc variants.
- **User auth:** JWT authenticates the user; org membership/role comes from Postgres state (no forwarded header trust).
- **User API contracts:** keep `docs/architecture/user_api_contracts.md` as the single owned inventory of `/v1/*` routes and their authz invariants. Do not implement or expose any user endpoint not listed there (default-deny).
- **Input filters (`where`):** structured map only (ADR 0007).
- **Buffered datasets:** pointer pattern + sink-side strict validation + row-level idempotency (ADR 0006).

### 3) Pick the v1 UDF language runners

Recommended for v1 (minimal but practical):
- Node runner (JS/TS)
- Python runner
- Rust runner (custom runtime via `cargo-lambda`)

A single DAG can mix languages by referencing different bundle IDs.

### 4) Freeze operational defaults

Do not leave “magic numbers” implicit. Defaults live in:
- `docs/standards/operations.md` (timings, limits, retry policies, runbooks)

Treat that file as the v1 “config skeleton.”

## Suggested milestones

This is a conservative build order that keeps each milestone independently testable.

### Milestone 0: Foundations
- Networking + IAM skeleton
- RDS: Postgres state + Postgres data
- Object storage: datasets/results/scratch buckets
- Queues: task wake-ups and dataset buffers (+ DLQs)

Exit: services boot and can reach dependencies.

### Milestone 1: Dispatcher core
- Postgres state schema: jobs, tasks, leases, retries, outbox
- Implement the core worker contracts in `docs/architecture/contracts.md`
- Implement outbox drain + retry policy (max attempts + backoff)

Exit: you can enqueue a task into Postgres state and see it wake a worker through the queue.

### Milestone 2: Platform worker wrapper (trusted)
- Poll queue, claim lease, heartbeat, complete
- Extend queue visibility while a task is running
- Emit task events via Dispatcher (attempt-fenced)

Exit: retries and restarts do not create double-commits.

### Milestone 3: Dataset versions + commit protocol
- Dataset registry + version pointers
- Replace/append staging layout (S3)
- Commit protocol is atomic and idempotent

Exit: failed attempts never become visible; retries produce a single committed version.

### Milestone 4: Buffered dataset sinks
- Implement ADR 0006 end-to-end (pointer publish → sink consumer → Postgres data)
- Strict batch parsing + DLQ on malformed input
- Stable row-level idempotency (`dedupe_key` + upsert)

Exit: duplicate publishes and task retries do not create duplicate rows.

### Milestone 5: Query Service + credential minting
- Task-scoped reads through Query Service (`/v1/task/query`)
- Scoped S3 credentials minted per task (`/v1/task/credentials`)
- Enforce query timeouts and export thresholds
- Implement and test the Query Service **SQL sandbox** (no filesystem/HTTP/URL reads, no extension loading, no user-supplied ATTACH/URIs)
- Implement the minimum feasible **PII access audit** model for Query Service (dataset-level; no raw SQL stored)

Exit: untrusted execution can read only allowed dataset versions and write only allowed prefixes.

### Milestone 6: UDF execution + alerting pipeline
- Platform-managed Lambda UDF runners (Node/Python/Rust)
- Alert evaluation emits `alert_events` batch artifacts
- Routing + Delivery Service process events

Exit: alerts are end-to-end functional with at-least-once semantics.

### Milestone 7: Production hardening
- Monitoring dashboards + alerts (based on `docs/standards/operations.md`)
- Runbooks: DLQ replay, outbox replay, staging cleanup, delivery retry
- Partition staleness detection and repair triggers (no silent gaps)
- Source liveness: heartbeat SLA + restart/backoff behavior for source runners

Exit: you can run a game day without manual SQL surgery.

## Trace Lite
For the local/dev profile, start with: `docs/plan/trace_lite.md`.
