# Interface Contracts

Component boundaries: task payloads, results, and upstream events.

## SQS → Worker

SQS message contains only `task_id`. Worker fetches full task details from the Dispatcher.

```json
{ "task_id": "uuid" }
```

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
- `input_datasets`: array of dataset names
- `output_datasets`: array of dataset names

When a task materializes outputs, it emits **one event per output dataset** (either batched or as separate requests).

Single-event shape:

```json
{ "dataset": "hot_blocks", "cursor": 12345 }
```

Partitioned shape:

```json
{ "dataset": "cold_blocks", "partition_key": "1000000-1010000" }
```

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

## Related

- [overview.md](overview.md) — system diagrams
- [orchestration.md](../capabilities/orchestration.md) — task/job schemas
- [data_versioning.md](data_versioning.md) — cursor and partition semantics
