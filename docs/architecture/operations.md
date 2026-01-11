# Operations

Doc ownership: this document defines Trace v1 operational targets, defaults, and runbooks.

Canonical architecture invariants live in:
- [invariants.md](invariants.md) - correctness and non-negotiable system truths
- [task_lifecycle.md](task_lifecycle.md) - leases, retries, outbox, and rehydration
- [security.md](security.md) - trust boundaries, auth model, and egress and secrets invariants

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
- See [security.md](security.md) for trust boundaries, auth model, and egress and secrets invariants.

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

## Retry safety and idempotency

Trace assumes **at-least-once** delivery. The system is correct if the invariants in [invariants.md](invariants.md) hold.

Key operational implications:
- **Task execution**: Postgres state is source of truth; queues are wake-ups only; attempt fencing is strict.
- **S3 outputs**: Workers write to staging; commit step records in Postgres state; readers only see committed.
- **Buffered datasets**: Idempotency via `dedupe_key` unique constraint.
- **External delivery**: At-least-once; receivers should dedupe; Delivery Service keeps a ledger.
- **Version retention**: Committed versions retained; v1 uses manual purge only.

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
| Query inline result cap (`inline_byte_limit`) | 10 MiB | Above this, export to object storage and return a URI |
| Query inline row cap (`inline_row_limit`) | 10,000 rows | Align with `/v1/query` `limit` clamp; larger results should export |
| Query presigned result URL expiry | 1h | User query exports only |
| Query concurrency cap (Lite) | 1 | Lite runs queries serially (single DuckDB connection behind a mutex) |
| Query concurrency cap (shared) | 3-5 | When enabling `/v1/query` for multiple users, add a small pool and backpressure; over cap, fall back to batch mode (`docs/specs/query_service_query_results.md`) |
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
