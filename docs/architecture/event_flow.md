# Event Flow

Reactive execution is event-driven, but durability comes from Postgres (state) plus leasing (see [task_lifecycle.md](task_lifecycle.md)).

```mermaid
sequenceDiagram
    participant Src as Source - Lambda or ECS
    participant D as Dispatcher
    participant PG as Postgres state
    participant TaskQ as SQS task queues
    participant W as Worker - ECS
    participant L as Lambda runtime
    participant BufQ as SQS dataset buffers
    participant Sink as Dataset Sink
    participant S as Storage - S3 or Postgres data

    Src->>D: POST /internal/events {dataset_uuid, dataset_version, cursor|partition_key}
    D->>PG: Persist event + enqueue routing work
    D->>PG: Create downstream tasks (dedupe)
    alt runtime == lambda
        D->>L: Invoke with full task payload
        L->>S: Execute, write output
        L->>D: POST /internal/task-complete {task_id, attempt, lease_token, status, events}
    else runtime == ecs_*
        D->>TaskQ: Enqueue wake-up {task_id}
        TaskQ->>W: Deliver {task_id}
        W->>D: POST /internal/task-claim {task_id, worker_id}
        D->>PG: Acquire lease (Queued -> Running)
        D->>W: {attempt, lease_token, task payload}
        W->>S: Execute, write output
        W->>D: POST /internal/task-complete {task_id, attempt, lease_token, status, events}
    end

    opt buffered Postgres dataset output (ADR 0006)
        Note over BufQ,Sink: Producers publish records to an SQS dataset buffer.\nThe sink drains, writes Postgres data, then emits a dataset event after commit.
        W->>BufQ: Publish records
        L->>BufQ: Publish records
        Sink->>BufQ: Drain messages
        Sink->>S: Write Postgres data
        Sink->>D: POST /internal/events after commit
    end
```

**Notes:**

- SQS is treated as unordered at-least-once. Workers must claim tasks (leases) before running.
- Workers extend SQS visibility for long tasks and heartbeat leases to Dispatcher.

## Related

- [contracts.md](contracts.md) — task/event payload shapes
- [task_lifecycle.md](task_lifecycle.md) — leasing, retries, rehydration
- [dispatcher.md](containers/dispatcher.md) — orchestration details
- [workers.md](containers/workers.md) — execution model
