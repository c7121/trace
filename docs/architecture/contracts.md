# Interface Contracts

Component boundaries: task payloads, results, and upstream events.

> `/internal/*` endpoints are internal-only and are not exposed to end users. They are called only by platform components (worker wrapper, operator runtimes, sinks, Delivery Service).


**Delivery semantics:** tasks and upstream events are **at-least-once**. Duplicates and out-of-order delivery are expected; correctness comes from attempt/lease gating plus idempotent output commits. See [task_lifecycle.md](task_lifecycle.md).

## SQS → Worker

SQS message contains only `task_id` (wake-up). The worker then claims the task from the Dispatcher to obtain a lease and the full task payload. Duplicates are expected.

```json
{ "task_id": "uuid" }
```

## Dispatcher → Lambda (runtime=lambda)

For jobs with `runtime: lambda`, the Dispatcher invokes the Lambda directly (no SQS). Invocation payload includes the **full task payload** (same shape as `/internal/task-fetch`) so the Lambda does not need to fetch task details before executing:

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "job": { "dag_name": "monad", "name": "block_follower" },
  "operator": "block_follower",
  "config": { "...": "..." },
  "inputs": [{ "...": "..." }]
}
```

Exact payload fields are still evolving; the invariant is that Lambda has everything it needs to run the operator without database credentials.

The Lambda follows the same worker contract: report completion/failure and emit events via the Dispatcher endpoints below. Task lifecycle (timeouts, retries) is described in [orchestration.md](data_model/orchestration.md).

Lambda built-in retries should be disabled; the Dispatcher owns retries/attempts uniformly across runtimes.

Small Lambda operators can be implemented in TypeScript/JavaScript, Rust, or Python.

## Worker → Dispatcher

Workers call Dispatcher for:
- Claim a task and obtain a lease + payload (`/internal/task-claim`)
- (Optional) Fetch task details (`/internal/task-fetch`)
- Report task completion/failure (`/internal/task-complete`)
- Heartbeat (`/internal/heartbeat`)
- Emit upstream events (`/internal/events`)

Workers never have state DB credentials.

Operator/UDF code must not be able to call `/internal/*`. The worker wrapper is the protection boundary: it authenticates to the Dispatcher, fetches task details, enforces the contract, and runs untrusted operator code with no internal credentials.

Secrets (when required) are injected at task launch (ECS task definition `secrets`) and are available to operator code as environment variables. Untrusted tasks must not have Secrets Manager permissions.

Event emission is explicit via `/internal/events` (mid-task) and may also be bundled as “final events” on `/internal/task-complete`.

Workers should only call `/internal/task-complete` after all intended events have been accepted (either emitted earlier via `/internal/events` or included as “final events” on `/internal/task-complete`).

Workers include `{task_id, attempt, lease_token}` on `/internal/heartbeat` and `/internal/task-complete`, and include `{task_id, attempt}` on `/internal/events`.
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
- **Credential Broker** — exchanges the token for short-lived STS credentials scoped to the allowed S3 prefixes.

UDF code never connects to Postgres directly.



### Task Claim (`/internal/task-claim`)

Workers must **claim** a task before executing operator/UDF code. Claiming acquires a short-lived lease so only one worker may run the current attempt.

```
POST /internal/task-claim
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

If not claimed, the worker should **not** execute the task and should ack/delete the SQS message.

### Task Fetch (`/internal/task-fetch`)

Workers fetch task details by `task_id` (read-only from the worker’s perspective):

```
GET /internal/task-fetch?task_id=<uuid>
```

If the task is canceled (e.g., during rollback), the Dispatcher may return `status: "Canceled"`.
In that case the wrapper exits without running operator code and reports the cancellation via `/internal/task-complete` with `status: "Canceled"`.

### Heartbeat (`/internal/heartbeat`)

Workers extend their lease while executing.

```
POST /internal/heartbeat
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
  "events": [{ "dataset_uuid": "uuid", "dataset_version": "uuid", "cursor": 12345 }]
}
```

Partitioned shape:

```json
{
  "task_id": "uuid",
  "attempt": 1,
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
  "events": [
    { "dataset_uuid": "uuid", "dataset_version": "uuid", "cursor": 12345 },
    { "dataset_uuid": "uuid", "dataset_version": "uuid", "cursor": 12345 }
  ]
}
```

Dispatcher routes events to dependent jobs based on the stored input edges (by upstream `dataset_uuid`).

Dispatcher treats events as at-least-once and idempotent. By default, it routes only events that refer to the dataset's **current** `dataset_version` (events for old generations may be accepted for audit but are not routed).

## Buffered Postgres Datasets (SQS Buffer → Sink → Postgres)

Some Postgres-backed datasets are written via a buffer + sink (NiFi-style “connection queue”):

- Producer jobs publish records to a dataset buffer (SQS).
- A platform-managed sink drains the buffer, writes the Postgres table, then emits the dataset event to the Dispatcher.
- This supports multi-writer datasets without granting producers Postgres write/DDL privileges.

### Producer → Dataset Buffer (SQS)

Message body (example):

```json
{
  "dataset_uuid": "uuid",
  "schema_hash": "sha256:...",
  "records": [
    {"org_id": "uuid", "dedupe_key": "job:10143:123", "severity": "warning", "payload": {"msg": "..."}, "created_at": "2025-12-27T12:00:00Z"}
  ]
}
```

The sink is responsible for idempotent writes (e.g., `UNIQUE (org_id, dedupe_key)`) and emitting the upstream event after commit.

## Related

- [readme.md](../readme.md) — system diagrams
- [orchestration.md](data_model/orchestration.md) — task/job schemas
- [data_versioning.md](data_versioning.md) — cursor and partition semantics
