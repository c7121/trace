# Plan

This directory is sequencing guidance. It is intentionally **not normative**; the source of truth for behavior and invariants is:
- `docs/architecture/*` (C4, contracts, lifecycle)
- `docs/specs/*` (public surfaces)
- `docs/adr/*` (decisions)

## Next steps (do these first)

### 1) Run a contract-freeze “happy path” test
Goal: validate that the task lifecycle contracts are implementable without gaps.

Deliverables:
- A stub Dispatcher with only:
  - `/internal/task-claim` (lease acquisition)
  - `/v1/task/heartbeat` (lease extension)
  - `/v1/task/complete` and `/v1/task/events` (attempt-fenced)
  - `/v1/task/buffer-publish` (attempt-fenced)
- A stub worker that:
  - consumes a `task_id` wake-up message,
  - claims a lease,
  - emits one event,
  - publishes one buffer batch pointer,
  - completes.

Exit criteria:
- You can kill/restart Dispatcher and worker and still get one committed completion (no double-commit).
- A stale attempt cannot complete or publish.

### 2) Freeze the “small but critical” public surfaces
Before building feature code, lock these decisions and test expectations:

- **Task auth model:** untrusted runtimes use **only** the per-attempt task capability token (+ lease fencing). No hidden shared secrets for Lambdas.
- **Input filters (`where`):** structured map only (ADR 0007).
- **User auth:** JWT authenticates user; org membership/role comes from Postgres state.

### 3) Pick the v1 language runners for UDF bundles
Recommended for v1 (minimal but practical):
- Node.js runner (JS/TS)
- Python runner
- Rust custom runtime runner (`bootstrap`)

A single DAG can mix languages by using different bundles.

### 4) Write down operational defaults (so implementation doesn’t guess)
These are “small numbers that become production problems” if left implicit:
- default lease duration + heartbeat interval
- capability token TTL and refresh/renew strategy
- outbox retry backoff and max attempts
- DLQ receive count and replay procedure
- retention policy for scratch/query exports

(If you don’t want a full spec, put them in `docs/standards/operations.md` as explicit defaults.)

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

Exit: you can enqueue a task into Postgres state and see it wake a worker through the queue.

### Milestone 2: Platform worker wrapper
- Poll queue, claim lease, heartbeat, complete
- Visibility timeout extension while a task is running

Exit: retries and restarts do not create double-commits.

### Milestone 3: Dataset versions + commit protocol
- Dataset registry + version pointers
- Replace/append staging layout

Exit: failed attempts never become visible; retries produce a single committed version.

### Milestone 4: Buffered dataset sinks
- Implement ADR 0006 end-to-end
- Strict batch parsing + DLQ on malformed input

Exit: duplicate publishes don’t create duplicate rows.

### Milestone 5: Query Service + credential minting
- Task-scoped reads through Query Service
- Scoped S3 credentials minted per task

Exit: untrusted execution can read only what it’s allowed to and write only to its allowed prefixes.

### Milestone 6: UDF execution + alerting pipeline
- Platform-managed Lambda UDF runners (Node/Python/Rust)
- Alert evaluation emits `alert_events` batches
- Routing + Delivery Service process events

Exit: alerts are end-to-end functional with at-least-once semantics.

### Milestone 7: Production hardening
- Autoscaling policies
- Operational runbooks (DLQ replay, retention, incident drills)

Exit: you can run a game day without manual SQL surgery.

## Trace Lite
For the local/dev profile, start with: `docs/plan/trace_lite.md`.
