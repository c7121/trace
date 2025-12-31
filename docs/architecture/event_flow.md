# Event Flow

```mermaid
sequenceDiagram
    participant Src as Source (Lambda/ECS)
    participant D as Dispatcher
    participant PG as Postgres (state)
    participant Q as SQS
    participant W as Worker
    participant L as Lambda (reactive job)
    participant Sink as Dataset Sink
    participant S as Storage

    Src->>D: Emit event {dataset_uuid, cursor}
    D->>PG: Find dependent jobs by input edges (dataset_uuid)
    D->>PG: Create tasks
    alt runtime == lambda
        D->>L: Invoke {task_id}
        L->>D: Fetch task details
        L->>S: Execute, write output (S3/Postgres)
        L->>D: Report status + emit event(s) {dataset_uuid, cursor|partition_key}
    else runtime == ecs_*
        D->>Q: Enqueue to operator queue
        Q->>W: Deliver task
        W->>D: Fetch task details
        W->>S: Execute, write output (S3/Postgres)
        W->>D: Report status + emit event(s) {dataset_uuid, cursor|partition_key}
    end

    opt buffered Postgres dataset output (ADR 0006)
        Note right of Sink: Producers publish records to an SQS buffer.\nThe sink writes Postgres and emits the upstream dataset event after commit.
        W->>Sink: Publish records (buffer)
        L->>Sink: Publish records (buffer)
        Sink->>S: Write Postgres table
        Sink->>D: Emit event(s) after commit
    end
```

**Flow:** Source emits → Dispatcher routes → Worker executes → Worker emits → repeat.

## Related

- [contracts.md](contracts.md) — task/event payload shapes
- [dispatcher.md](containers/dispatcher.md) — orchestration details
- [workers.md](containers/workers.md) — execution model
- [ADR 0006](adr/0006-buffered-postgres-datasets.md) — buffered Postgres datasets

