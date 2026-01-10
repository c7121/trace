# Buffered Postgres datasets contract

Some published datasets are written by multiple jobs (for example `alert_events`). In v1, untrusted tasks must never write to Postgres data directly.

This document defines the Queue to sink to table contract used by buffered datasets.

## Pattern

1. Producer writes a batch artifact to object storage (S3 or MinIO) under a buffer prefix.
2. Producer calls `POST /v1/task/buffer-publish` (attempt-fenced) with a pointer to that artifact.
3. Dispatcher persists the publish request (outbox) and enqueues a small message to the Buffer Queue (SQS in AWS, pgqueue in Lite).
4. A trusted sink worker (`ecs_platform`) dequeues the message, validates schema, performs an idempotent upsert into Postgres data, and emits the dataset event via Dispatcher.
5. The batch artifact can be GC'd by policy after the sink commits.

## Buffer Queue message (example)

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid",
  "batch_uri": "s3://trace-scratch/buffers/{task_id}/{attempt}/batch.jsonl",
  "content_type": "application/jsonl",
  "batch_size_bytes": 123456,
  "dedupe_scope": "alert_events"
}
```

Notes:
- Queue messages must remain small (<256KB). Do not embed full records in queue messages.
- Row-level idempotency is required. Buffered dataset rows must include a deterministic idempotency key (for example `dedupe_key`) or a natural unique key that is stable across retries. The sink enforces this with `UNIQUE(...)` plus `ON CONFLICT DO NOTHING/UPDATE`.
- Duplicates across attempts are expected. Batch artifacts may be written per-attempt to avoid object key collisions; correctness comes from sink-side row dedupe, not from attempt numbers.
- Multi-tenant safety (future AWS profile): the sink must assign `org_id` from a trusted publish record or queue message and must not trust `org_id` values embedded inside batch rows.
- Producers do not need direct access to the queue backend; the Dispatcher owns queue publishing via the outbox.

## Related

- Task-scoped buffer publish endpoint: [task_scoped_endpoints.md](task_scoped_endpoints.md)
- Alerting spec: [alerting.md](../../specs/alerting.md)
- Buffered datasets ADR: [0006-buffered-postgres-datasets.md](../../adr/0006-buffered-postgres-datasets.md)

