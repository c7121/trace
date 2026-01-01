# Failure Drills

Game day drills validate that Trace behaves correctly under failures, restarts, and duplicate delivery.
Run these in a staging environment with production-like settings.

This document is intentionally concise. It focuses on observable outcomes and the invariants that must hold.
See: [task_lifecycle.md](../architecture/task_lifecycle.md), [data_versioning.md](../architecture/data_versioning.md), and [idempotency.md](idempotency.md).

## Drills

### 1. Restart Dispatcher under load

**Inject:** restart the Dispatcher while tasks are queued and workers are running.  
**Expect:** no tasks are lost; workers retry and eventually complete.  
**Verify:** queued tasks resume via outbox and SQS wake-ups; no duplicate commits.

### 2. Kill a worker mid-task before completion

**Inject:** terminate a worker after it has claimed a lease and started running.  
**Expect:** the lease expires; the task is retried with a new attempt.  
**Verify:** outputs from the dead attempt are not committed; the new attempt commits successfully.

### 3. Duplicate SQS delivery for the same task

**Inject:** replay a task wake-up message or force SQS redelivery.  
**Expect:** only one worker acquires the lease; the other fails claim and drops work.  
**Verify:** single attempt executes; no concurrent execution.

### 4. Outbox publisher down

**Inject:** stop the outbox publisher (not the Dispatcher API) while creating tasks.  
**Expect:** tasks are durable in Postgres state; they are not lost.  
**Verify:** once publisher returns, backlogged outbox entries enqueue wake-ups and tasks run.

### 5. Postgres state outage

**Inject:** block connectivity to Postgres state (or failover).  
**Expect:** Dispatcher returns errors; workers back off; no split brain.  
**Verify:** after recovery, leases and retries resume; no stale attempt can commit.

### 6. Sink worker down with buffered datasets

**Inject:** stop dataset sink workers while producers publish buffer messages.  
**Expect:** buffer queues grow; nothing is silently dropped.  
**Verify:** after sink restart, rows are written once (dedupe enforced).

### 7. Query Service overload

**Inject:** run many heavy queries or artificially reduce query service capacity.  
**Expect:** queries degrade gracefully and limits are enforced.  
**Verify:** timeouts and limits are enforced; Postgres data is protected (read replica recommended).

### 8. Credential Broker unavailable

**Inject:** stop the Credential Broker.  
**Expect:** tasks that require scoped S3 creds fail fast and retry; no fallback to broad IAM.  
**Verify:** system remains secure; no direct secrets access is introduced.

### 9. Delivery provider timeouts

**Inject:** simulate webhook timeouts or a provider outage.  
**Expect:** delivery retries occur; delivery ledger prevents unbounded duplication from platform retries.  
**Verify:** eventual delivery or failure recorded; idempotency key is included on each attempt.

## What to record

For each drill, capture:

- the triggering event and timestamps
- logs for Dispatcher, workers, and the relevant service
- queue depth and age (task queues and dataset buffers)
- Postgres state transitions for the task(s)
- evidence that committed outputs are unique and attempt-fenced

