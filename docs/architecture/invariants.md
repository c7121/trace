# System invariants

This document owns the **minimal, enforceable truths** of the Trace platform.
Other docs may explain rationale, but MUST NOT contradict these invariants.

If you are implementing or reviewing code, read this first.

## Design principles (v1)

- Everything is a job - streaming services, batch transforms, and checks are modeled as jobs
- Everything produces assets - outputs are recorded as identifiable assets (tables, objects, URIs)
- Workers are dumb - workers execute tasks and report results; orchestration lives in the Dispatcher and Postgres state
- YAML is source of truth - definitions live in git; state lives in Postgres state
- Single Dispatcher service - orchestration is centralized and restartable; durability comes from Postgres state

Related:
- Operator specs: [../specs/operators/README.md](../specs/operators/README.md)
- DAG schema: [../specs/dag_configuration.md](../specs/dag_configuration.md)
- Dispatcher container: [containers/dispatcher.md](containers/dispatcher.md)
- Data versioning: [data_versioning.md](data_versioning.md)

## Correctness under failure

- **Execution is at-least-once** end-to-end. Duplicate delivery is always possible.
- **Idempotency is required** for all state transitions and sinks:
  - task completion is idempotent,
  - buffered dataset publish is idempotent,
  - any external side-effect must be recorded and de-duped by a stable key.
- **Attempts are fenced**:
  - any mutation that “finishes” or “publishes” a task is fenced by `(task_id, attempt, lease_token)`,
  - stale attempts MUST be rejected.
- **Leasing is enforced** for mutations:
  - only the current lease holder may write completion/events/buffer-publish,
  - leases expire; workers can crash and be retried.
- **Outbox drives side-effects**:
  - a Dispatcher restart must not lose “what to enqueue/invoke next”,
  - outbox delivery is at-least-once; consumers must tolerate duplicates.

See: [task_lifecycle.md](task_lifecycle.md), [contracts.md](contracts.md).

## Queues

- Queues are **wake-ups**, not sources of truth.
- Messages may be duplicated, delayed, or delivered out of order.
- The system MUST be correct if a queue is temporarily unavailable (work will resume when it recovers).

## Data model and boundaries

- **Postgres state** is the system of record for orchestration and lineage.
- **Postgres data** stores user-facing datasets and platform-owned query/alert tables.
- There are **no cross-DB foreign keys**. Cross-DB references are **soft** and must be validated at read/write boundaries.
- Any “soft ref” (`org_id`, `user_id`, `task_id`, etc.) MUST be treated as untrusted input and checked against the caller’s identity context.

See: [db_boundaries.md](db_boundaries.md), [data_model/README.md](data_model/README.md), ADRs [0008](../adr/0008-dataset-registry-and-publishing.md) and [0009](../adr/0009-atomic-cutover-and-query-pinning.md).

## Trust boundaries

- User-supplied code (UDFs) is **untrusted** by default.
- Untrusted code MUST NOT receive long-lived platform credentials (Postgres writers, broad S3 access, internal service auth).
- Untrusted code may only use:
  - **task-scoped endpoints** (`/v1/task/*`) authenticated by a **task capability token**, and
  - Query Service task API (`/v1/task/query`) which is itself fail-closed and sandboxed.
- `/internal/*` endpoints are **internal-only** and must not be reachable from untrusted runtimes.

See: [security.md](security.md), [contracts.md](contracts.md).

## Query safety

- Query execution must be **fail-closed**:
  - unsafe SQL is rejected before execution (`trace-core::query::validate_sql`),
  - runtime must not enable network readers or extension install/load,
  - results and PII access are audited without storing raw SQL.

See: [../specs/query_sql_gating.md](../specs/query_sql_gating.md), [containers/query_service.md](containers/query_service.md).
