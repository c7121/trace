# Task-scoped endpoint contracts

Task-scoped endpoints are callable by untrusted execution (for example `runtime: lambda` UDF runner) and by trusted operator runtimes. They are authenticated with:
- `X-Trace-Task-Capability: <capability_token>` (short-lived JWT), and
- `{task_id, attempt, lease_token}` in the request body (must match the token claims and the current lease).

The Dispatcher accepts task-scoped calls only for the current attempt and current lease. Stale attempts MUST be rejected and must not commit outputs or mutate state. See [task_lifecycle.md](../task_lifecycle.md).

Workers never have Postgres state credentials.

Untrusted code may call only task-scoped endpoints for its own attempt. It must not be able to call privileged platform endpoints (admin APIs, cross-task mutations, queue publishing, secrets).

## Secrets (platform operators only)

Secrets (when required) are injected at task launch (ECS task definition `secrets`) and are available to operator code as environment variables. Untrusted tasks must not have Secrets Manager permissions.

## Buffered dataset publish (`/v1/task/buffer-publish`)

Buffered Postgres datasets are published by calling `POST /v1/task/buffer-publish`. This is attempt-fenced just like heartbeat and completion.

```
POST /v1/task/buffer-publish
X-Trace-Task-Capability: <capability_token>
```

Request:

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

Dispatcher behavior:
- Persist a buffered publish record and enqueue a Buffer Queue message via the outbox (atomic with Postgres state).
- Reject if `(task_id, attempt, lease_token)` does not match the current lease.
- Treat duplicate publishes as idempotent (same `(task_id, attempt, batch_uri)`).

See [buffered_datasets.md](buffered_datasets.md) for the sink-side queue and write contract.

## Heartbeat (`/v1/task/heartbeat`)

Workers extend their lease while executing.

```
POST /v1/task/heartbeat
X-Trace-Task-Capability: <capability_token>
```

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid"
}
```

Dispatcher rejects heartbeats for stale attempts or stale lease tokens.

## Task completion (`/v1/task/complete`)

```
POST /v1/task/complete
X-Trace-Task-Capability: <capability_token>
```

Task completion includes an `outputs` array so a single task can materialize multiple outputs. Outputs are referenced internally by `dataset_uuid` (and optionally `output_index`).

For Parquet dataset publishing (replace-style outputs), tasks may also include a `datasets_published` list. This is the minimal publication shape needed to register `dataset_versions` deterministically:
- `dataset_uuid`, `dataset_version` (pinned)
- `storage_ref` (version-addressed storage reference for Parquet objects; manifest is optional and Trace-owned)
- optional metadata such as `config_hash` and `{range_start, range_end}` for range-based datasets (for example Cryo bootstrap)

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
  "datasets_published": [
    {
      "dataset_uuid": "uuid",
      "dataset_version": "uuid",
      "storage_ref": {
        "scheme": "s3",
        "bucket": "bucket",
        "prefix": "cold/datasets/{dataset_uuid}/{dataset_version}/",
        "glob": "*.parquet"
      },
      "config_hash": "string",
      "range_start": 100,
      "range_end": 200
    }
  ],
  "error_message": null
}
```

Event emission is explicit via `POST /v1/task/events` (mid-task) and may also be bundled as final events on `POST /v1/task/complete`.

Workers should call `POST /v1/task/complete` only after all intended events have been accepted (either emitted earlier via `POST /v1/task/events` or included in `events` on completion).

Late replies for the current attempt may still be accepted even if the task was already marked timed out (as long as no newer attempt has started).

## Upstream events (`/v1/task/events`)

```
POST /v1/task/events
X-Trace-Task-Capability: <capability_token>
```

Jobs can produce multiple outputs. DAG wiring in YAML is by `{job, output_index}` edges, but at runtime the Dispatcher routes by the upstream output identity (`dataset_uuid`).

Input filters are read-time predicates applied by the consumer. See [ADR 0007](../../adr/0007-input-edge-filters.md).

YAML example:

```yaml
inputs:
  - from: { dataset: alert_events }
    where:
      severity: critical
```

When a task materializes outputs, it emits one event per output (either batched or as separate requests).

Single-event shape:

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid",
  "events": [{ "dataset_uuid": "uuid", "dataset_version": "uuid", "cursor": 12345 }]
}
```

Partitioned shape:

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid",
  "events": [{ "dataset_uuid": "uuid", "dataset_version": "uuid", "partition_key": "1000000-1010000", "start": 1000000, "end": 1010000 }]
}
```

For block-range partitions, `partition_key` is `{start}-{end}` (inclusive).

For Parquet datasets (especially Cryo-derived datasets), keep the `{start}_{end}` range in the Parquet object key or filename (for example `blocks_{start}_{end}.parquet`) for interoperability and debugging. The dataset root or prefix is still resolved via the registry and may be UUID-based (for example `.../dataset/{dataset_uuid}/version/{dataset_version}/...`).

Batch shape:

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid",
  "events": [
    { "dataset_uuid": "uuid", "dataset_version": "uuid", "cursor": 12345 },
    { "dataset_uuid": "uuid", "dataset_version": "uuid", "cursor": 12345 }
  ]
}
```

Dispatcher routes events to dependent jobs based on the stored input edges (by upstream `dataset_uuid`).

Dispatcher treats events as at-least-once and idempotent. By default, it routes only events that refer to the dataset current `dataset_version` (events for old generations may be accepted for audit but are not routed).

Producer identity: upstream events are associated with a producing `task_id` and an `attempt`. The `task_id` is durable across retries and can be treated as a `producer_task_id` run id for idempotency and auditing. For long-running sources, the source runtime should preserve a stable producer run id across restarts whenever feasible (treat restarts like retries of the same run).

## Related

- Task capability token: [task_capability_tokens.md](task_capability_tokens.md)
- Worker-only endpoints: [worker_only_endpoints.md](worker_only_endpoints.md)
- Buffered datasets: [buffered_datasets.md](buffered_datasets.md)
