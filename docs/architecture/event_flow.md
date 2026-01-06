# Event Flow

Reactive execution is event-driven, but durability comes from Postgres state plus leasing.

```mermaid
sequenceDiagram
    participant Src as Source - Lambda or ECS
    participant D as Dispatcher
    participant PG as Postgres state
    participant TaskQ as SQS task queues
    participant W as Worker - ECS
    participant L as Lambda runtime
    participant BufQ as SQS dataset buffers
    participant SinkW as Platform worker sink operator
    participant S as Storage - S3 or Postgres data

    Src->>D: Emit upstream events, fenced
    D->>PG: Persist event and enqueue routing work
    D->>PG: Create downstream tasks dedupe
    alt runtime is lambda
        D->>L: Invoke with full task payload
        L->>S: Execute, write output to staging
        L->>D: Report task completion, fenced
        D->>PG: Commit outputs and advance cursor or record partition
        D->>D: Emit and route dataset events after commit
    else runtime is ecs
        D->>TaskQ: Enqueue wake up task_id
        TaskQ->>W: Deliver task_id
        W->>D: Claim task, acquire lease
        D->>PG: Acquire lease Queued to Running
        D->>W: Return attempt, lease_token, task payload
        W->>S: Execute, write output to staging
        W->>D: Report task completion, fenced
        D->>PG: Commit outputs and advance cursor or record partition
        D->>D: Emit and route dataset events after commit
    end

    opt buffered Postgres dataset output
        W->>S: Write batch artifact to scratch
        W->>D: Publish buffer batch pointer, fenced
        L->>S: Write batch artifact to scratch
        L->>D: Publish buffer batch pointer, fenced
        D->>BufQ: Enqueue pointer message
        SinkW->>BufQ: Drain messages
        SinkW->>S: Write Postgres data
        SinkW->>D: Emit dataset events after commit, fenced
    end
```


**Notes:**

- Dispatcher side effects (enqueue tasks, route events) are executed via the Postgres state outbox worker; the diagram shows the logical effects.

- SQS is treated as unordered at-least-once. Workers must claim tasks (leases) before running.
- Workers extend SQS visibility for long tasks and heartbeat leases to Dispatcher.
- For `replace` outputs to S3, workers write to a staging prefix and the Dispatcher commits the output (metadata) before routing events.

