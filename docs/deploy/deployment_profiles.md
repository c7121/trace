# Deployment Profiles

Trace supports two deployment profiles:

- **AWS (production)**: S3 + SQS + Postgres state + Postgres data.
- **Trace Lite (desktop/dev)**: MinIO + pgqueue + Postgres while preserving the same orchestration semantics.

The design intent is that **Dispatcher core logic and task lifecycle behavior are identical** between profiles, to avoid codepath drift.

## Non-goals

- Trace Lite is not a security sandbox. Auth and permissions may be permissive.
- Lite does not attempt IAM/STS/VPC parity.

## Core invariants

These invariants are required in **all** profiles:

1. **Postgres state is the source of truth** for orchestration:
   - tasks, attempts, leases, heartbeats
   - outbox entries
   - dataset commit metadata and active pointers
2. **Queue delivery is at-least-once**:
   - duplicates may occur
   - ordering is not guaranteed
   - correctness must not depend on FIFO
3. **Queue messages are wake-ups, not authority**:
   - workers must claim work via Postgres state leasing
4. **Strict attempt fencing**:
   - any output commit includes `(task_id, attempt)`
   - the Dispatcher rejects commits for stale attempts
5. **Outbox is required**:
   - durable intent + outbox row are written in the same DB transaction
   - an outbox publisher executes the side effect (enqueue) later

## Adapter matrix

| Capability | AWS | Trace Lite |
|---|---|---|
| Object store (cold Parquet) | S3 | MinIO (S3-compatible) |
| Queue backend | SQS (Standard) | pgqueue (Postgres-backed queue) |
| Postgres state | RDS Postgres | Postgres container (db/schema) |
| Postgres data | RDS Postgres | Postgres container (db/schema) |
| Cron triggers | EventBridge Scheduler/Rules -> Lambda | compose `scheduler` container or local cron -> HTTP |
| Webhooks ingress | API Gateway -> Lambda or Gateway | Gateway HTTP directly |

## Strict rule: Dispatcher core is identical

**Normative requirement:** The Dispatcher must never enqueue directly as part of creating tasks or buffered work.

The only allowed flow is:

1) write durable intent (task row / buffer publish row)
2) write an outbox row describing the enqueue
3) commit
4) outbox publisher later calls `QueueDriver.publish(...)`

This applies even in Trace Lite where pgqueue lives in Postgres.
Do not optimize Lite by inserting into `queue_messages` in the same transaction as task creation.

Publisher implementation note:
- the outbox publisher may run as a separate process/container or as a loop inside the Dispatcher
- outbox rows must be marked as sent in a separate transaction from the intent-creation transaction

## One queue abstraction

All internal queue use cases use the same QueueDriver interface (task wake-ups, buffered datasets, delivery work).

### QueueDriver operations

Required operations:

- `publish(queue_name, payload_json, delay_seconds=0)`
- `receive(queue_name, max_messages, visibility_timeout_seconds) -> [Message]`
- `ack(queue_name, receipt)`

Recommended:

- `extend_visibility(queue_name, receipt, visibility_timeout_seconds)`

Where `Message` includes:

- `payload_json`
- `receipt` (opaque handle used for ack/extend)
- `delivery_count` (best-effort)

### Required semantics

- at-least-once delivery
- duplicates allowed
- no ordering guarantee
- visibility timeout supported

Poison handling:

- AWS: SQS redrive policy to DLQ
- Lite: pgqueue `max_attempts` moved to dead table

## Queue payload shapes

Keep payloads small and typed:

- Task wake-up: `{"kind":"task_wakeup","task_id":"<uuid>"}`
- Buffered batch: `{"kind":"buffer_batch","dataset_uuid":"<uuid>","dataset_version":"<uuid>","batch_uri":"s3://...","record_count":123}`
- Delivery work: `{"kind":"delivery","delivery_id":"<uuid>"}`

## pgqueue backend (Trace Lite)

pgqueue is a Postgres-backed QueueDriver used for desktop installs, CI, and demos.
It is not intended as the primary production queue.

### Minimal DDL

```sql
CREATE TABLE queue_messages (
  id            BIGSERIAL PRIMARY KEY,
  queue_name    TEXT NOT NULL,
  payload       JSONB NOT NULL,

  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  visible_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

  lease_until   TIMESTAMPTZ,
  lease_token   UUID,

  attempts      INT NOT NULL DEFAULT 0,
  max_attempts  INT NOT NULL DEFAULT 20,

  last_error    TEXT
);

CREATE INDEX queue_ready_idx
  ON queue_messages (queue_name, visible_at, id);

CREATE INDEX queue_lease_idx
  ON queue_messages (queue_name, lease_until);

CREATE TABLE queue_dead (
  id           BIGINT PRIMARY KEY,
  queue_name   TEXT NOT NULL,
  payload      JSONB NOT NULL,
  created_at   TIMESTAMPTZ NOT NULL,
  dead_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  attempts     INT NOT NULL,
  last_error   TEXT
);
```

### Receive and lease algorithm (conceptual)

Select available rows and lock:

```sql
SELECT id, payload
FROM queue_messages
WHERE queue_name = $1
  AND visible_at <= now()
  AND (lease_until IS NULL OR lease_until < now())
  AND attempts < max_attempts
ORDER BY id
LIMIT $2
FOR UPDATE SKIP LOCKED;
```

Lease the selected rows:

```sql
UPDATE queue_messages
SET lease_until = now() + ($3 || ' seconds')::interval,
    lease_token = gen_random_uuid(),
    attempts = attempts + 1
WHERE id = ANY($ids)
RETURNING id, payload, lease_token, attempts;
```

Return `receipt = {id, lease_token}`.

### Ack

```sql
DELETE FROM queue_messages
WHERE id = $1 AND lease_token = $2;
```

### Dead-lettering

```sql
WITH moved AS (
  DELETE FROM queue_messages
  WHERE attempts >= max_attempts
  RETURNING id, queue_name, payload, created_at, attempts, last_error
)
INSERT INTO queue_dead (id, queue_name, payload, created_at, attempts, last_error)
SELECT id, queue_name, payload, created_at, attempts, last_error FROM moved;
```

## Trace Lite docker compose

A minimal Lite stack should be runnable with `docker compose up` and include:

- `postgres` (state + data)
- `minio` (S3-compatible object store)
- `gateway`
- `dispatcher` (runs the outbox publisher loop, or runs alongside a separate publisher)
- `query_service`
- `platform_worker`
- optional `delivery_service` (webhook demo)
- optional `rpc_egress_gateway` (RPC calls for demo)
- optional `scheduler` (cron-like triggers for demo DAGs)

## Triggers

### Trace Lite

- **Cron**: a `scheduler` container (or local cron) that calls a Dispatcher endpoint.
- **Webhook**: the Gateway exposes HTTP endpoints directly.

Both must translate to the same internal behavior:

- write durable intent + outbox row in Postgres state
- outbox publisher enqueues wake-ups via QueueDriver

### AWS

- **Cron**: EventBridge Scheduler/Rules -> Lambda -> Dispatcher enqueue
- **Webhook**: API Gateway -> Lambda (or Gateway) -> Dispatcher enqueue

The Dispatcher core remains unchanged; only the trigger adapter differs.

## Trace Lite build plan

The build plan and demo scope are defined in `docs/plan/trace_lite.md`.
