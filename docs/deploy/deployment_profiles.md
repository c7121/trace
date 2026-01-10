# Deployment Profiles

Trace supports two deployment profiles:

- **AWS (production)**: S3 + SQS + Postgres state + Postgres data.
- **Trace Lite (desktop/dev)**: MinIO + pgqueue + Postgres while preserving the same orchestration semantics.

The design intent is that **Dispatcher core logic and task lifecycle behavior are identical** between profiles, to avoid codepath drift.

## Non-goals

- Trace Lite is not a security sandbox. Auth and permissions may be permissive.
- Lite does not attempt IAM/STS/VPC parity.

## What stays the same

Core execution invariants do not change between profiles. Start here:

- Task lifecycle and outbox: [docs/architecture/task_lifecycle.md](../architecture/task_lifecycle.md)
- System invariants: [docs/architecture/invariants.md](../architecture/invariants.md)

## Adapter matrix

| Capability | AWS | Trace Lite |
|---|---|---|
| Object store (cold Parquet) | S3 | MinIO (S3-compatible) |
| Queue backend | SQS (Standard) | pgqueue (Postgres-backed queue) |
| Postgres state | RDS Postgres | Postgres container (db/schema) |
| Postgres data | RDS Postgres | Postgres container (db/schema) |
| Cron triggers | EventBridge Scheduler/Rules -> Lambda | compose `scheduler` container or local cron -> HTTP |
| Webhooks ingress | API Gateway -> Lambda or Gateway | Gateway HTTP directly |

## Trace Lite details

Keep Lite details anchored in one place:

- pgqueue schema: [harness/migrations/state/0001_init.sql](../../harness/migrations/state/0001_init.sql)
- Harness assumptions: [harness/NOTES.md](../../harness/NOTES.md)
- trace-lite runner: [docs/plan/trace_lite.md](../plan/trace_lite.md)
- End-to-end local sync example: [docs/examples/lite_local_cryo_sync.md](../examples/lite_local_cryo_sync.md)
