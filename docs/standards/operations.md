# Operations

This document defines Trace v1 operational targets and the invariants that must hold under failures, restarts, duplicates, and partial outages.

It is intentionally **self-contained** so operators and engineers can reason about correctness without jumping across many files.

## Non-functional requirements

### Timeliness
- Near real-time data availability for alerting use cases.
- Alerts must not be missed; delays must be observable and bounded by backlog.

### Data integrity
- No silent data loss.
- Gaps/corruption must be detectable and recoverable.
- Outputs must be verifiable (manifests, checksums, and/or deterministic partition identity).

### Reliability
- At-least-once execution is acceptable; correctness comes from idempotent effects and attempt fencing.
- Control-plane state is durable across restarts.

### Security
- Untrusted UDFs have no direct Postgres access and no direct internet egress.
- Secrets are injected at launch (not fetched by untrusted code).
- Outbound communication is centralized in egress services (Delivery Service, RPC Egress Gateway).

### Observability
- Every unit of work must be attributable to an org/job/task.
- Backlog must be measurable (queue depth/age, lease lag, sink lag).
- Failures must be visible (DLQs, retries, error rates, stale leases).

### Cost discipline
- Query and scan costs must be attributable (dataset bytes scanned, S3 I/O, Postgres read load).
- Prefer “export to S3” for large query results instead of streaming giant payloads.


## Resource limits and quotas

These controls exist to protect platform stability and costs. They are not security boundaries, but they are part of the operational contract.

- **CPU/memory**: hard caps in ECS task definitions / Lambda memory settings.
- **Execution timeout**: platform-enforced maximum runtime per job.
- **Disk quota**: ephemeral storage capped per task.
- **Rate limits**: max concurrent jobs and jobs-per-hour per org.
- **Cost alerts**: alert when an org approaches spend thresholds.

## Retry safety and idempotency invariants

Trace assumes **at-least-once** delivery for both tasks and internal events. The system is correct if the following invariants hold.

### Task execution
- **Postgres state is the source of truth** for task status and leases; queues are wake-ups only.
- A task attempt is **single-runner** via leasing (claim → heartbeat → complete).
- **Attempt fencing is strict**: stale attempts must not be able to commit outputs or advance pointers.

### Replace-style S3 outputs
For dataset outputs written to S3 (Parquet):
- Workers write to a unique **staging location** for `(task_id, attempt, partition_key)`.
- A separate **commit step** records the committed location/manifest in Postgres state.
- Readers only read **committed** locations.
- Cleaning up abandoned staging data is allowed, but must never delete committed outputs.

### Buffered Postgres datasets
For multi-writer or restricted-write datasets:
- Producers publish records to **SQS dataset buffers**.
- A trusted sink consumer writes to **Postgres data**.
- Idempotency is enforced using a stable `dedupe_key` (unique constraint + upsert/do-nothing).

### External delivery
Delivery is the main intentional exception where duplicates may be externally visible:
- Delivery is at-least-once; receivers should dedupe when possible.
- The Delivery Service keeps a delivery ledger keyed by a stable idempotency key.
- Webhooks are **POST-only** in v1.

### Version retention
- **Committed** dataset versions are retained by default; v1 uses **manual** purge only.
- Staging cleanup (uncommitted attempt outputs) is a separate concern from committed version retention.

## Failure drills (game day)

Run these drills in staging with production-like configuration.

For each drill: inject the failure, then verify correctness (no missed data, no stale commits, observable backlog, recovery).

1. **Restart Dispatcher**
   - Expect: tasks remain durable; leases rehydrate; outbox resumes.
2. **Kill a worker mid-task**
   - Expect: lease expires; task retries; outputs are attempt-fenced (no stale commit).
3. **Duplicate task wake-ups**
   - Expect: only one attempt claims; duplicates are harmless.
4. **Pause Postgres state briefly**
   - Expect: workers fail fast or back off; no silent corruption; recovery resumes.
5. **Pause Postgres data briefly**
   - Expect: sinks and query service degrade gracefully; backlog increases; recovery drains.
6. **Break SQS temporarily**
   - Expect: outbox retries; tasks are not lost; backlog observable.
7. **Poison buffer message**
   - Expect: DLQ capture; sink continues; manual replay path exists.
8. **Delivery retries**
   - Expect: at-least-once behavior; ledger prevents unbounded duplicate sends; failures are observable.

## Default timings and limits (v1)

These defaults exist to prevent “it worked in dev” drift. They are starting points, not a promise.

| Knob | Default | Notes |
|------|---------|-------|
| Task lease duration | 120s | Short enough to recover quickly; long enough to avoid heartbeat storms |
| Heartbeat interval | 30s | Aim for ~4 heartbeats per lease period |
| Capability token TTL | `timeout_seconds + 5m` (cap 24h) | Token lifetime must cover the task timeout; scope is per-task attempt |
| Task max attempts | 3 | Default when omitted in DAG YAML |
| Task retry backoff | exponential (30s → 10m) + jitter | `next_retry_at` is owned by Dispatcher |
| Outbox max attempts | 20 | After this, mark the row `Failed` and alert |
| Outbox backoff | exponential (1s → 5m) + jitter | `available_at` controls when it is eligible again |
| Buffer DLQ max receives | 10 | After this, messages require manual inspection/replay |
| Buffer batch artifact max size | 16 MiB | Keep batch artifacts small; shard by partition if needed |
| Query max runtime (user) | 60s | For `/v1/query`; encourage export for larger work |
| Query max runtime (task) | 300s | For `/v1/task/query`; tasks must chunk work if longer |
| Query inline result cap | 10 MiB | Above this, export to object storage and return a URI |
| Scratch/query export retention | 7d | Keep short; make it explicit |
| Staging output retention | 7d | Uncommitted attempt outputs may be GC’d; never delete committed outputs |
| Delivery max attempts | 12 | After this, mark delivery terminal-failed and alert |
| Delivery backoff | exponential (5s → 30m) + jitter | Per-destination retry policy; avoid retry storms |
| User API rate limit (per user) | 100 req/min | Enforced at API Gateway; backends still validate JWTs |
| User API rate limit (per org) | 1000 req/min | v1 is single-org but keep the knob for future expansion |
| Query rate limit (per user) | 10 req/min | `/v1/query` is expensive; prefer export for large work |

Invariants:
- `capability_token_ttl` MUST be >= `timeout_seconds` (or tasks can fail late on auth).
- Heartbeats MUST be frequent enough that 1–2 missed heartbeats does not immediately kill a healthy task.
- Outbox side effects MUST be retried until `Done` or terminal `Failed` with an alert; do not silently drop.
- If Query Service exports results, the export path MUST be idempotent (content-addressed or attempt-scoped).

### Suggested formulas (v1)

Keep these as defaults, but make them configurable.

- **Exponential backoff with jitter**:
  - `delay = min(max_delay, base_delay * 2^attempt) * rand(0.5..1.5)`
- **Task retries**:
  - `base_delay = 30s`, `max_delay = 10m`
- **Outbox retries**:
  - `base_delay = 1s`, `max_delay = 5m`
- **Delivery retries**:
  - `base_delay = 5s`, `max_delay = 30m`

### Operational runbooks (minimum)

These are the “you will need this in week 2” procedures. Keep them simple.

- **Outbox failed rows**: inspect `outbox.status='Failed'`, fix root cause, then reset to `Pending` (with a comment) to replay.
- **Buffer DLQ replay**: drain DLQ to a quarantine bucket, validate/repair, then republish pointers (never republish raw records).
- **Staging cleanup**: delete only uncommitted attempt prefixes older than retention; never delete committed manifests/pointers.
- **Delivery terminal failures**: expose a “retry delivery” admin action that requeues a delivery with a new attempt counter (do not mutate history).
