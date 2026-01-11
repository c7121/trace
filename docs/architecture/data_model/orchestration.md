# Orchestration Data Model

Schema mapping notes for Postgres state tables that back orchestration: identity, DAG deploys, jobs, datasets, tasks, and lineage.

Where to look:
- Columns: [state_schema.md](state_schema.md)
- Relationships: [erd_state.md](erd_state.md)
- Behavior: [task_lifecycle.md](../task_lifecycle.md), [DAG Configuration](../../specs/dag_configuration.md)
- Data versioning mapping: [data_versioning.md](data_versioning.md)

> Canonical DDL (when a table is implemented in harness) lives in `harness/migrations/state/` (applied in order).

## Identity

Tables: `orgs`, `users`, `org_roles`, `org_role_memberships`.

Invariants:
- `orgs.slug` is unique.
- `users.external_id` is unique and holds the IdP subject.
- `org_roles` is unique by `(org_id, slug)`.
- `org_role_memberships` primary key is `(role_id, user_id)`. Membership lookup is indexed by `user_id`.

## Deploys

Tables: `dag_versions`, `dag_current_versions`.

Invariants:
- `dag_versions` is unique by `(org_id, dag_name, yaml_hash)`.
- `dag_current_versions` primary key is `(org_id, dag_name)` and points to the serving `dag_version_id`.

## Jobs

Table: `jobs`.

Invariants:
- Uniqueness: `(dag_version_id, name)`.
- `runtime` drives execution mode:
  - `ecs_*`: queue-woken worker claims a task, then executes.
  - `lambda`: Dispatcher acquires the lease and invokes Lambda directly.
  - `dispatcher`: Dispatcher runs the operator in-process.
- `execution_strategy` declares incremental mode (`PerUpdate` or `PerPartition`).
- `bootstrap` is source-only and may request output resets (see [data_versioning.md](../data_versioning.md)).

## Dataset registry

Tables: `datasets`, `dataset_versions`, `dag_version_datasets`.

Invariants:
- `datasets`:
  - `(org_id, name)` is unique.
  - `(org_id, producer_dag_name, producer_job_name, producer_output_index)` is unique (single-producer enforcement).
- `dataset_versions`:
  - Determinism: `(dataset_uuid, config_hash, range_start, range_end)` is unique.
- `dag_version_datasets` primary key is `(dag_version_id, dataset_uuid)`.

Notes:
- For S3/Parquet datasets, `storage_location` is a version-resolved prefix (ending in `/`) that contains Parquet objects.
- Query Service attaches datasets by prefix + glob and fails closed on authz mismatch.
- Cryo output must not require a Trace-owned manifest. Manifests are an optional optimization.

## Tasks and outbox

Tables: `tasks`, `task_inputs`, `outbox`, `operator_state`.

Invariants:
- Task lifecycle semantics are defined in [task_lifecycle.md](../task_lifecycle.md) and fenced by task-scoped endpoint contracts ([task_scoped_endpoints.md](../contracts/task_scoped_endpoints.md)).
- Idempotent task creation: when `dedupe_key` is set, tasks are unique by `(job_id, dedupe_key)`.
- Expected indexes: `status` (Queued/Running), `next_retry_at` (Failed), `lease_expires_at` (Running)
- Outbox is required for crash-safe side effects:
  - any durable mutation that needs enqueueing or routing writes an outbox row in the same transaction
  - pending scan is driven by `(status, available_at)` for status `Pending`
- `task_inputs` primary key is `(task_id, input_dataset_uuid, input_partition_key)`.
- `operator_state` primary key is `(org_id, job_id, state_key)`.

## Lineage

Table: `column_lineage` (optional).

Invariants:
- Primary key: `(output_dataset_uuid, output_column, input_dataset_uuid, input_column)`.

## Related

- [Architecture Overview](../../README.md) - system design and component diagrams
- [DAG Configuration](../../specs/dag_configuration.md) - YAML schema
- [DAG Deployment](../dag_deployment.md) - deploy/sync flow
- [lambda_invocation.md](../contracts/lambda_invocation.md) - Dispatcher-to-Lambda payload shape
