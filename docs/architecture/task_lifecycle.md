# Task Lifecycle

This document defines the durable execution model for tasks: how tasks are created, claimed, retried, and recovered after failures.

**Summary:** **Postgres state** is the source of truth. Side effects are recorded in an **outbox**. **SQS** is a wake-up mechanism.

## Guarantees

- **At-least-once delivery**: SQS may deliver duplicates; workers may retry calls; the platform may restart.
- **Single active attempt**: for a given `task_id`, only one attempt is considered current.
- **No concurrent execution**: a task is executed only by the worker that holds the current lease.
- **Rehydratable**: after Dispatcher restarts, queued work resumes without losing tasks.

These guarantees are achieved by **leasing** (in Postgres state) plus **idempotent output commit** (replace/append + unique keys).

## Mental Model

- **Postgres state** stores tasks, attempts, leases, and retry scheduling.
- **SQS** delivers a pointer (`task_id`) so workers don't poll Postgres.
- **Workers are dumb**: they do not decide retries or scheduling. They:
  1) receive a `task_id` from SQS,
  2) claim the task (acquire a lease) from Dispatcher,
  3) execute,
  4) heartbeat,
  5) complete.

If SQS loses a message or redelivers duplicates, the system still works because the task row remains in Postgres state.

## Task States

Tasks move through these states:

- `Queued`: eligible to be claimed.
- `Running`: currently leased by a worker.
- `Completed`: finished successfully.
- `Failed`: finished unsuccessfully and may be retried.
- `Canceled`: explicitly canceled (e.g., during rollback).

> **Note:** SQS delivery is not a task state. A task can be `Queued` in Postgres state even if no SQS message exists.

## Leasing

A **lease** is a time-bounded right to execute the current attempt of a task.

- Lease fields live on the task row (see `orchestration.md`).
- The lease has:
  - `lease_token` (opaque UUID)
  - `lease_expires_at`
  - `worker_id` (current holder)

### Claim

When a worker receives a `task_id` from SQS it calls:

- `POST /internal/task-claim`

Dispatcher performs an atomic transition:

- `Queued -> Running`
- sets `lease_token`, `lease_expires_at`, `worker_id`, `started_at`

If the task is already `Running` with a valid lease, or is `Completed/Canceled`, the claim is rejected.

**Rule:** the worker must not execute operator code unless it successfully claimed the task.

### Heartbeat

While running, the worker periodically heartbeats:

- `POST /internal/heartbeat {task_id, attempt, lease_token}`

Dispatcher extends `lease_expires_at` if the lease token matches the current attempt.

### Completion

On completion the worker reports:

- `POST /internal/task-complete {task_id, attempt, lease_token, status, outputs, events}`

Dispatcher accepts completion only if:

- `attempt` matches the task's current attempt, and
- `lease_token` matches the current lease token.

This prevents stale completions from prior attempts from mutating state (including output commits).

## SQS Visibility

SQS visibility timeout is not a correctness mechanism; the lease is.

Workers **must extend** message visibility for long-running tasks.

- Default queue visibility can be relatively short (minutes).
- The worker wrapper calls `ChangeMessageVisibility` periodically until completion.

If the worker dies:

- the lease expires,
- the reaper reschedules the task (see below), and
- a fresh SQS message is published for the same `task_id`.

## Retries

Retries are owned by the Dispatcher.

- Each task has `max_attempts`.
- On failure or timeout, Dispatcher marks the task `Failed` and sets `next_retry_at` with backoff.
- When the retry becomes eligible, Dispatcher transitions `Failed -> Queued`, increments `attempt`, and republishes a wake-up message.

A retry does **not** create a new task row.

## Background Loops

Three loops make the system rehydratable:

1) **Outbox worker**
   - drains `outbox` rows created by Dispatcher transactions
   - performs side effects: enqueue SQS wake-ups (`enqueue_task`) and route upstream events (`route_event`)
   - on restart, resumes from `Pending` rows (no lost work)

2) **Retry scheduler**
   - finds tasks eligible to retry (`status='Failed'` and `next_retry_at <= now()`)
   - transitions them back to `Queued`, increments `attempt`, and writes an `enqueue_task` outbox row

3) **Lease reaper**
   - finds tasks with expired leases (`status='Running'` and `lease_expires_at < now()`)
   - marks them timed out and schedules a retry (or terminal failure)

If SQS drops a wake-up, the task is still durable in Postgres state; it can be safely re-enqueued by writing another `enqueue_task` outbox row (leasing prevents concurrent execution).



## Ordering

Ordering is enforced by **DAG dependencies** and **dataset versioning**, not by queue ordering.

- SQS is treated as unordered at-least-once.
- Schema evolution is a first-class ETL concern: jobs may add/rename/recode fields as part of their output.
- Correctness comes from pinning to `{dataset_uuid, dataset_version}` and committing outputs atomically; avoid in-place DDL on shared Postgres tables (especially from untrusted code).

## Related

- [contracts.md](contracts.md) — endpoints and payload shapes
- [orchestration.md](data_model/orchestration.md) — task schema
- [dispatcher.md](containers/dispatcher.md) — orchestration responsibilities
