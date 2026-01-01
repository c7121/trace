# Idempotency

Trace is an **at-least-once** system. SQS can deliver duplicates, services can restart, and workers can retry calls.
Correctness comes from **attempt fencing** and **idempotent side effects**.

This checklist is the single place that enumerates side-effect boundaries and how they stay safe under retries.

See also: [task_lifecycle.md](../architecture/task_lifecycle.md), [data_versioning.md](../architecture/data_versioning.md), and ADRs [0006](../architecture/adr/0006-buffered-postgres-datasets.md) and [0009](../architecture/adr/0009-atomic-cutover-and-query-pinning.md).

## Core rules

1. **Every task has an `attempt`.** Only the current attempt may commit outputs.
2. **Write, then commit.** Workers write outputs to staging locations; the Dispatcher commits by moving pointers or metadata in Postgres state.
3. **Side effects are scoped and attributable.** Every externally visible action must be tied to a `task_id` and `attempt` (and usually an idempotency key).

## Side-effect boundaries

| Boundary | Owner | Idempotency key | Enforcement |
|---|---|---|---|
| Task claim and lease | Dispatcher + worker | `(task_id, attempt)` + `lease_token` | Atomic lease acquisition in **Postgres state**; heartbeats extend lease |
| Task completion | Worker | `(task_id, attempt)` + `lease_token` | Dispatcher rejects stale attempts; only current attempt can transition state |
| Replace outputs on S3 | Worker writes, Dispatcher commits | `(dataset_uuid, dataset_version, partition_key)` + `(task_id, attempt)` | Write to staging prefix; commit pointer update is attempt-fenced |
| Append outputs on S3 | Worker writes, Dispatcher commits | `(dataset_uuid, dataset_version, partition_key)` + `(task_id, attempt)` | Staging + commit; readers only consume committed manifests or locations |
| Buffered datasets into Postgres data | Producers + sink worker | `dedupe_key` | Sink enforces unique constraint or upsert (ADR 0006) |
| Query exports to S3 | Query Service | `query_id` or `(task_id, attempt, query_id)` | Export location is deterministic; overwrites are safe because results are immutable once referenced |
| Outbound delivery | Delivery Service | `delivery_id` and per-channel idempotency key | Delivery ledger in **Postgres data**; retries are allowed; downstream may still deliver duplicates |
| Credential minting | Credential Broker | token subject `(org_id, task_id, attempt)` | Repeated mint calls are safe; STS creds are short-lived and scope-restricted |

### Notes

- **Delivery is the exception.** Trace guarantees at-least-once attempts; exactly-once delivery depends on the downstream provider supporting idempotency.
- **Staging cleanup is not version GC.** v1 uses manual GC for committed dataset versions (ADR 0009). Uncommitted staging artifacts may be cleaned up without deleting committed outputs.

## Operator author checklist

When adding a new operator or sink, ensure:

- Inputs are pinned to a **dataset version** (never “latest” without an explicit policy).
- Outputs use either:
  - **replace**: deterministic partition scope + staging + commit, or
  - **append**: stable `dedupe_key` + sink-side unique constraint.
- Any external call includes an **idempotency key** and a durable ledger (preferably via a buffered dataset and a dedicated service).

