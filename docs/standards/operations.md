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
