# Database Boundaries (Postgres state vs Postgres data)

Trace uses **two separate Postgres instances/clusters**:

- **Postgres state**: control-plane source of truth (orgs/users, DAG versions, jobs, tasks, leases, outbox, dataset registry, dataset versions, invalidations).
- **Postgres data**: data-plane tables (chain hot tables, alerting tables, query results, buffered dataset sinks, PII audit log).

The split is primarily a **blast-radius and operational safety** boundary: heavy queries, ingestion bursts, or table bloat in Postgres data must not take down scheduling/leases in Postgres state.

> Assumption: deployments often scope access at the **instance** level (not per-table). Treat the instance boundary as the primary security and blast-radius boundary.

## Invariants

1. **Postgres state is authoritative.** If it isn’t durable in Postgres state, it doesn’t exist.
2. **Scheduling must not depend on Postgres data availability.** If Postgres data is down, the Dispatcher may degrade UX and data APIs, but it must still be able to lease/retry/cancel tasks and perform rollbacks.
3. **No cross-database foreign keys.** Two RDS instances cannot enforce referential integrity across them.

## Soft references (no cross-DB FKs)

Tables in Postgres data may include columns like `org_id`, `user_id`, `producer_job_id`, `producer_task_id` for attribution and filtering.

These are **soft references** to Postgres state identifiers. Do **not** write DDL like:

```sql
org_id UUID REFERENCES orgs(id)
```

Instead, use a plain column and document the relationship:

```sql
org_id UUID NOT NULL -- soft ref: Postgres state orgs(id)
```

## Application-level integrity plan (minimal)

Because Postgres cannot enforce cross-DB integrity, Trace enforces correctness by **controlling writers** and **carrying trusted context**:

- **API-created rows (CRUD)**: the Dispatcher is the writer and validates identifiers (e.g., org/user existence) in Postgres state before writing to Postgres data.
- **Buffered datasets**: the Dispatcher accepts a fenced `/v1/task/buffer-publish` from the task lease-holder and enqueues a buffer message that includes trusted `org_id` and producer identifiers.
  - The sink worker must assign `org_id` (and other attribution) from the trusted buffer message / publish record.
  - The sink must not trust `org_id` embedded inside batch rows written by user code.
- **Other platform writers** (trusted operators/sinks): treated as trusted code. If they write inconsistent IDs, it is a platform bug, not a tenant bypass.

Optional (recommended) reconciliation:

- Periodically (nightly) scan Postgres data for orphaned `org_id`/`user_id` values and emit metrics/logs.
- Reconciliation is not in the scheduling critical path; it is an audit/quality check.

## If you need DB-enforced integrity later

If you decide soft references are insufficient, there are two real options:

- **Single Postgres instance** with separate schemas/roles/resource controls (lower ops, weaker blast-radius isolation).
- **Replicate reference tables** (`orgs`, `users`, `jobs`, `tasks`) into Postgres data for FK enforcement (more moving parts; drift/failure modes).

## Related

- Control-plane tables: [orchestration.md](data_model/orchestration.md), [data_versioning.md](data_model/data_versioning.md)
- Data-plane tables: [alerting.md](data_model/alerting.md), [query_service.md](data_model/query_service.md), [address_labels.md](data_model/address_labels.md)
