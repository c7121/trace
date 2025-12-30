# Interface Contracts

Component boundaries: task payloads, results, and upstream events.

## SQS → Worker

SQS message contains only `task_id`. Worker fetches full task details from the Dispatcher.

```json
{ "task_id": "uuid" }
```

## Dispatcher → Lambda (runtime=lambda)

For jobs with `runtime: lambda`, the Dispatcher invokes the Lambda directly (no SQS). Invocation payload includes only `task_id`:

```json
{ "task_id": "uuid" }
```

The Lambda follows the same worker contract: fetch task details and report completion/failure via the Dispatcher endpoints below. Task lifecycle (timeouts, retries) is described in [orchestration.md](../capabilities/orchestration.md).

## Worker → Dispatcher

Workers call Dispatcher for:
- Fetch task details (`/internal/task-fetch`)
- Report task completion/failure (`/internal/task-complete`)
- Heartbeat (`/internal/heartbeat`)
- Emit upstream events (`/internal/events`)

Workers never have state DB credentials.

## Task Completion (Worker → Dispatcher)

Task completion includes an `outputs` array so a single task can materialize multiple datasets.

```json
{
  "task_id": "uuid",
  "status": "Completed",
  "outputs": [
    { "dataset": "hot_blocks", "location": "postgres://hot_blocks", "cursor": 12345, "row_count": 1000 },
    { "dataset": "hot_logs", "location": "postgres://hot_logs", "cursor": 12345, "row_count": 20000 }
  ],
  "error_message": null
}
```

## Upstream Events (Worker → Dispatcher)

Jobs can produce multiple datasets. DAG wiring is therefore:
- `input_datasets`: array of dataset names (optionally with per-input filters)
- `output_datasets`: array of dataset names

Input filters are read-time predicates applied by the consumer (Dispatcher still routes by dataset name only). See [ADR 0007](adr/0007-input-edge-filters.md).

YAML example:

```yaml
input_datasets:
  - name: alert_events
    where: "severity = 'critical'"
```

When a task materializes outputs, it emits **one event per output dataset** (either batched or as separate requests).

Single-event shape:

```json
{ "dataset": "hot_blocks", "cursor": 12345 }
```

Partitioned shape:

```json
{ "dataset": "cold_blocks", "partition_key": "1000000-1010000" }
```

For block-range partitions, `partition_key` is `{start}-{end}` (inclusive) and maps to Cryo-style Parquet filenames `{dataset}_{start}_{end}.parquet`.

Batch shape:

```json
{
  "events": [
    { "dataset": "hot_blocks", "cursor": 12345 },
    { "dataset": "hot_logs", "cursor": 12345 }
  ]
}
```

Dispatcher routes events to dependent jobs based on `input_datasets`.

## Buffered Postgres Datasets (SQS Buffer → Sink → Postgres)

Some Postgres-backed datasets are written via a buffer + sink (NiFi-style “connection queue”):

- Producer jobs publish records to a dataset buffer (SQS).
- A platform-managed sink drains the buffer, writes the Postgres table, then emits the dataset event to the Dispatcher.
- This supports multi-writer datasets without granting producers Postgres write/DDL privileges.

### Producer → Dataset Buffer (SQS)

Message body (example):

```json
{
  "dataset": "alert_events",
  "schema_hash": "sha256:...",
  "records": [
    {"org_id": "uuid", "dedupe_key": "job:10143:123", "severity": "warning", "payload": {"msg": "..."}, "created_at": "2025-12-27T12:00:00Z"}
  ]
}
```

The sink is responsible for idempotent writes (e.g., `UNIQUE (org_id, dedupe_key)`) and emitting the upstream event after commit.

## Related

- [readme.md](../readme.md) — system diagrams
- [orchestration.md](../capabilities/orchestration.md) — task/job schemas
- [data_versioning.md](data_versioning.md) — cursor and partition semantics
