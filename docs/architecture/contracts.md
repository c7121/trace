# Interface Contracts

Component boundaries: task payloads, results, and upstream events.

> `/internal/*` endpoints are internal-only and are not exposed to end users. They are callable only by **trusted** platform components (worker wrappers and platform services). Untrusted runtimes (UDF code, including `runtime: lambda`) must not call `/internal/*`.
>
> **Transport:** TLS is required for all internal APIs.
>
> **Auth model:**
> - **Task-scoped endpoints** (heartbeat/complete/events/buffer publish) are authenticated with a short-lived **task capability token** plus `{task_id, attempt, lease_token}` fencing.
> - **Worker-only endpoints** (task claim) are callable only by trusted worker wrappers and are protected by network policy plus a worker identity mechanism (see below).
> - **Privileged platform endpoints** (if any) should use a separate service identity mechanism (recommended: service JWT); **mTLS is optional hardening**, not a requirement.

**Task capability token format (v1):**
- The task capability token is a **JWT signed by the Dispatcher** (recommended: ES256).
- Verifiers (Dispatcher `/v1/task/*`, Query Service `/v1/task/query`, sinks) validate signature and expiry using the Dispatcher’s internal **task-JWKS** document.
- The task-JWKS endpoint is internal-only (e.g., `GET /internal/jwks/task`) and should be cached by verifiers; rotation uses `kid`.


**Delivery semantics:** tasks and upstream events are **at-least-once**. Duplicates and out-of-order delivery are expected; correctness comes from attempt/lease gating plus idempotent output commits. See [task_lifecycle.md](task_lifecycle.md).

> **Internal-only:** endpoints under `/v1/task/*` are reachable only from within the VPC (workers/Lambdas) and must not be routed through the public Gateway.


## Queue → Worker (Task wake-up)

Task queue message contains only `task_id` (wake-up). The worker then claims the task from the Dispatcher to obtain a lease and the full task payload. Duplicates are expected.

```json
{ "task_id": "uuid" }
```

## Dispatcher → Lambda (runtime=lambda)

For jobs with `runtime: lambda`, the Dispatcher invokes the Lambda directly (no task queue).

In AWS, `runtime: lambda` should refer to a **platform-managed UDF runner** Lambda (per environment), not user-deployed Lambdas.
- The runner treats the bundle as untrusted code.
- The runner’s execution role should be near-zero (no broad S3/SQS/Secrets Manager access).
- The Dispatcher should supply an object-scoped **pre-signed S3 GET URL** for the bundle so the runner does not need S3 IAM permissions.

> `runtime: lambda` may execute either platform operators or untrusted UDF bundles. Treat Lambda as **untrusted by default**: do not rely on hidden internal credentials.
>
> For task execution, the Dispatcher includes a per-attempt **task capability token** in the invocation payload. The Lambda uses that token to:
> - read data via Query Service (`/v1/task/query`),
> - obtain scoped S3 credentials (`/v1/task/credentials`), and
> - report fenced heartbeat/completion/events to the Dispatcher.

Invocation payload includes the **full task payload** (same shape as `/internal/task-fetch`) so the Lambda does not need to fetch task details before executing:

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid",
  "lease_expires_at": "2025-12-31T12:00:00Z",
  "capability_token": "jwt",
  "bundle_url": "https://s3.../udf/{bundle}.zip?X-Amz-...",
  "job": { "dag_name": "monad", "name": "block_follower" },
  "operator": "block_follower",
  "config": { "...": "..." },
  "inputs": [{ "...": "..." }]
}
```

Exact payload fields are still evolving; the invariant is that Lambda has everything it needs to run the operator and report fenced completion without Postgres state credentials.

Dispatcher acquires a lease before invoking (transitions the task to Running) and includes `(attempt, lease_token)` in the payload.

The Lambda follows the same worker contract: heartbeat (optional) and report completion/failure and events via the Dispatcher endpoints below. Task lifecycle (timeouts, retries) is defined in [task_lifecycle.md](task_lifecycle.md).

Lambda built-in retries should be disabled; the Dispatcher owns retries/attempts uniformly across runtimes.

Small Lambda operators can be implemented in TypeScript/JavaScript, Rust, or Python.

## Worker → Dispatcher

Workers call Dispatcher for:
- Claim a task and obtain a lease + payload (`/internal/task-claim`)
- (Optional) Fetch task details (`/internal/task-fetch`)
- Report task completion/failure (`/v1/task/complete`)
- Heartbeat (`/v1/task/heartbeat`)
- Emit upstream events (`/v1/task/events`)

Workers never have Postgres state credentials.

**Untrusted code may call only task-scoped endpoints for its own attempt.**
It must not be able to call privileged platform endpoints (admin APIs, cross-task mutations, queue publishing, secrets).

Authentication is split by endpoint type:

- **Task-scoped endpoints** (`/v1/task/heartbeat`, `/v1/task/complete`, `/v1/task/events`, `/v1/task/buffer-publish`) require:
  - `X-Trace-Task-Capability: <capability_token>` (short-lived JWT), and
  - `{task_id, attempt, lease_token}` in the request body (must match the token + the current lease).

- **Worker-only endpoints** (`/internal/task-claim`, `/internal/task-fetch`) are called only by trusted worker wrappers (ECS pollers).
  They must be protected by network policy (security groups allow only worker services) and a worker identity mechanism (e.g., `X-Trace-Worker-Token` injected only into the wrapper container).

For `runtime: lambda`, the Lambda receives the task capability token in the invocation payload and uses it directly. There is no wrapper boundary in Lambda; do not rely on hidden shared secrets in Lambda.

AWS note: ECS/Fargate tasks do not support per-container IAM roles. If you execute untrusted UDF code in ECS, you must ensure it does not share AWS API permissions (SQS, queue ack, etc.) with the wrapper/poller.


Secrets (when required) are injected at task launch (ECS task definition `secrets`) and are available to operator code as environment variables. Untrusted tasks must not have Secrets Manager permissions.

Event emission is explicit via `/v1/task/events` (mid-task) and may also be bundled as “final events” on `/v1/task/complete`.

Workers should only call `/v1/task/complete` after all intended events have been accepted (either emitted earlier via `/v1/task/events` or included as “final events” on `/v1/task/complete`).

Workers include `{task_id, attempt, lease_token}` on `/v1/task/heartbeat`, `/v1/task/complete`, and `/v1/task/events`.
The Dispatcher accepts these calls only for the **current** attempt and current lease; stale attempts are rejected and **must not** commit outputs or mutate state. See [task_lifecycle.md](task_lifecycle.md).

Late replies for the current attempt may still be accepted even if the task was already marked timed out (as long as no newer attempt has started).

Producer identity: upstream events are associated with a producing `task_id` and an `attempt`. The `task_id` is durable across retries and can be treated as a `producer_task_id`/run ID for idempotency and auditing. For long-running sources, the source runtime should preserve a stable producer run ID across restarts whenever feasible (treat restarts like retries of the same run).

### UDF Data Access Token (Capability Token)

For **untrusted UDF tasks**, the Dispatcher issues a short-lived **capability token** (passed to the runtime as an env var such as `TRACE_TASK_CAPABILITY_TOKEN`).

The token is the single source of truth for what the UDF is allowed to read and write during the attempt:

- Allowed input datasets (pinned dataset versions) and their resolved storage locations
- Allowed output prefix (S3)
- Allowed scratch/export prefix (S3)

The token is enforced by:

- **Query Service** — for ad-hoc SQL reads across Postgres + S3; only the datasets in the token are attached as views.
- **Dispatcher** — exchanges the token for short-lived STS credentials scoped to the allowed S3 prefixes (credential minting).

In ECS, a trusted **worker wrapper** typically performs task/lease calls and credential minting and then injects the resulting scoped credentials into the untrusted process.

In `runtime: lambda`, the Lambda uses the capability token directly (there is no wrapper boundary).

UDF code never connects to Postgres directly.



### Task Claim (`/internal/task-claim`)

Workers must **claim** a task before executing operator/UDF code. Claiming acquires a short-lived lease so only one worker may run the current attempt.

```
POST /internal/task-claim
X-Trace-Worker-Token: <worker_token>
```

Request:

```json
{
  "task_id": "uuid",
  "worker_id": "ecs:cluster/service/task"
}
```

Response (claimed):

```json
{
  "status": "Claimed",
  "attempt": 1,
  "lease_token": "uuid",
  "lease_expires_at": "2025-12-31T12:00:00Z",
  "capability_token": "jwt",
  "task": {
    "task_id": "uuid",
    "attempt": 1,
    "job": { "dag_name": "monad", "name": "block_follower" },
    "operator": "block_follower",
    "config": { "...": "..." },
    "inputs": [{ "...": "..." }]
  }
}
```

Response (not claimed):

```json
{ "status": "NotClaimed", "reason": "AlreadyRunning|Completed|Canceled|NotFound" }
```

If not claimed, the worker should **not** execute the task and should ack/delete the queue message.

### Task Fetch (`/internal/task-fetch`)

Workers fetch task details by `task_id` (read-only from the worker’s perspective):

```
GET /internal/task-fetch?task_id=<uuid>
X-Trace-Worker-Token: <worker_token>
```

If the task is canceled (e.g., during rollback), the Dispatcher may return `status: "Canceled"`.
In that case the wrapper exits without running operator code and reports the cancellation via `/v1/task/complete` with `status: "Canceled"`.

## Buffered dataset publish (Worker → Dispatcher)

Buffered Postgres datasets are published by calling `POST /v1/task/buffer-publish`. This is attempt-fenced just like heartbeat/completion.

```
POST /v1/task/buffer-publish
X-Trace-Task-Capability: <capability_token>
```

Request:

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid",
  "dataset_uuid": "uuid",
  "dataset_version": "uuid",
  "batch_uri": "s3://trace-scratch/buffers/{dataset_uuid}/{task_id}/{attempt}/batch.jsonl",
  "record_count": 1000
}
```

Dispatcher behavior:

- Persist a buffered publish record and enqueue a Buffer Queue message via the outbox (atomic with Postgres state).
- Reject if `(task_id, attempt, lease_token)` does not match the current lease.
- Treat duplicate publishes as idempotent (same `(task_id, attempt, dataset_uuid, batch_uri)`).

### Heartbeat (`/v1/task/heartbeat`)

Workers extend their lease while executing.

```
POST /v1/task/heartbeat
X-Trace-Task-Capability: <capability_token>
```

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid"
}
```

Dispatcher rejects heartbeats for stale attempts or stale lease tokens.

## Task Completion (Worker → Dispatcher)

```
POST /v1/task/complete
X-Trace-Task-Capability: <capability_token>
```

Task completion includes an `outputs` array so a single task can materialize multiple outputs. Outputs are referenced internally by `dataset_uuid` (and optionally `output_index`).

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid",
  "status": "Completed",
  "events": [
    { "dataset_uuid": "uuid", "dataset_version": "uuid", "cursor": 12345 }
  ],
  "outputs": [
    { "output_index": 0, "dataset_uuid": "uuid", "dataset_version": "uuid", "location": "postgres_table:dataset_{dataset_uuid}", "cursor": 12345, "row_count": 1000 },
    { "output_index": 1, "dataset_uuid": "uuid", "dataset_version": "uuid", "location": "postgres_table:dataset_{dataset_uuid}", "cursor": 12345, "row_count": 20000 }
  ],
  "error_message": null
}
```

## Upstream Events (Worker → Dispatcher)

```
POST /v1/task/events
X-Trace-Task-Capability: <capability_token>
```

Jobs can produce multiple outputs. DAG wiring in YAML is by `{job, output_index}` edges, but at runtime the Dispatcher routes by the upstream output identity (`dataset_uuid`).

Input filters are read-time predicates applied by the consumer. See [ADR 0007](adr/0007-input-edge-filters.md).

YAML example:

```yaml
inputs:
  - from: { dataset: alert_events }
    where: "severity = 'critical'"
```

When a task materializes outputs, it emits **one event per output** (either batched or as separate requests).

Single-event shape:

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid",
  "events": [{ "dataset_uuid": "uuid", "dataset_version": "uuid", "cursor": 12345 }]
}
```

Partitioned shape:

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid",
  "events": [{ "dataset_uuid": "uuid", "dataset_version": "uuid", "partition_key": "1000000-1010000", "start": 1000000, "end": 1010000 }]
}
```

For block-range partitions, `partition_key` is `{start}-{end}` (inclusive).

For Parquet datasets (especially Cryo-derived datasets), keep the `{start}_{end}` range in the Parquet object key / filename (e.g., `blocks_{start}_{end}.parquet`) for interoperability and debugging. The dataset root/prefix is still resolved via the registry and may be UUID-based (e.g., `.../dataset/{dataset_uuid}/version/{dataset_version}/...`).

Batch shape:

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid",
  "events": [
    { "dataset_uuid": "uuid", "dataset_version": "uuid", "cursor": 12345 },
    { "dataset_uuid": "uuid", "dataset_version": "uuid", "cursor": 12345 }
  ]
}
```

Dispatcher routes events to dependent jobs based on the stored input edges (by upstream `dataset_uuid`).

Dispatcher treats events as at-least-once and idempotent. By default, it routes only events that refer to the dataset's **current** `dataset_version` (events for old generations may be accepted for audit but are not routed).

## Buffered Postgres Datasets (Queue → sink → table)

Some published datasets are written by multiple jobs (e.g., `alert_events`). In v1, **untrusted tasks must never write to Postgres data directly**.

Pattern:

1. Producer writes a batch artifact to object storage (S3/MinIO) under a buffer prefix.
2. Producer calls `POST /v1/task/buffer-publish` (attempt-fenced) with a pointer to that artifact.
3. Dispatcher persists the publish request (outbox) and enqueues a small message to the **Buffer Queue** (Queue in AWS, pgqueue in Lite).
4. A trusted sink worker (`ecs_platform`) dequeues the message, validates schema, performs an idempotent upsert into Postgres data, and emits the dataset event via Dispatcher.
5. The batch artifact can be GC’d by policy after the sink commits.

Queue message (example):

```json
{
  "kind": "buffer_batch",
  "org_id": "uuid",
  "dataset_uuid": "uuid",
  "dataset_version": "uuid",
  "batch_uri": "s3://trace-scratch/buffers/{dataset_uuid}/{task_id}/{attempt}/batch.jsonl",
  "record_count": 1000,
  "producer": { "task_id": "uuid", "attempt": 1 }
}
```

Notes:

- Queue messages must remain small (<256KB). **Do not embed full records** in queue messages.
- **Row-level idempotency is required.** Buffered dataset rows must include a deterministic idempotency key (e.g., `dedupe_key`) or a natural unique key that is stable across retries. The sink enforces this with `UNIQUE(...)` + `ON CONFLICT DO NOTHING/UPDATE`.
- Duplicates across attempts are expected. Batch artifacts may be written per-attempt to avoid S3 key collisions; correctness comes from sink-side row dedupe, not from attempt numbers.
- **Multi-tenant safety:** the sink must assign `org_id` from the trusted publish record / queue message and must not trust `org_id` values embedded inside batch rows.
- Producers do not need direct access to the queue backend; the Dispatcher owns queue publishing via the outbox.

