# Event Flow

```mermaid
sequenceDiagram
    participant Src as Source (Lambda/ECS)
    participant D as Dispatcher
    participant PG as Postgres (state)
    participant TaskQ as SQS (task queues)
    participant W as Worker (ECS)
    participant L as Lambda (runtime=lambda)
    participant BufQ as SQS (dataset buffers)
    participant Sink as Dataset Sink
    participant S as Storage

    Src->>D: POST /internal/events {dataset_uuid, cursor|partition_key}
    D->>PG: Find dependent jobs by input edges (dataset_uuid)
    D->>PG: Create tasks
    alt runtime == lambda
        D->>L: Invoke with full task payload {task_id, ...}
        L->>S: Execute, write output (S3/Postgres)
        L->>D: POST /internal/task-complete {status, events}
    else runtime == ecs_*
        D->>TaskQ: Enqueue {task_id}
        TaskQ->>W: Deliver {task_id}
        W->>D: GET /internal/task-fetch?task_id=uuid
        D->>W: Task payload {operator, config, inputs, ...}
        W->>S: Execute, write output (S3/Postgres)
        W->>D: POST /internal/task-complete {status, events}
    end

    opt buffered Postgres dataset output (ADR 0006)
        Note over BufQ,Sink: Producers publish records to an SQS dataset buffer.\nThe sink drains the buffer, writes Postgres, and emits the dataset event after commit.
        W->>BufQ: Publish records (buffer)
        L->>BufQ: Publish records (buffer)
        Sink->>BufQ: Drain messages
        Sink->>S: Write Postgres table
        Sink->>D: POST /internal/events after commit
    end
```

**Flow:** Source emits → Dispatcher routes → Worker/Lambda executes → emit events → repeat.

## Related

- [contracts.md](contracts.md) — task/event payload shapes
- [dispatcher.md](containers/dispatcher.md) — orchestration details
- [workers.md](containers/workers.md) — execution model
- [ADR 0006](adr/0006-buffered-postgres-datasets.md) — buffered Postgres datasets
