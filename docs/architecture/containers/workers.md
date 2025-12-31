# Workers

Executors. One worker image per runtime.

## Component View

```mermaid
flowchart LR
    subgraph Worker["Worker Container"]
        wrapper["Worker Wrapper"]:::component
        operator["Operator (job code)"]:::component
    end

    sqs["SQS"]:::infra
    buffers["Dataset Buffers (SQS)"]:::infra
    dispatcher["Dispatcher"]:::component
    postgres["Postgres"]:::database
    s3["S3"]:::database
    secrets["Secrets Manager"]:::infra
    rpc["RPC Providers"]:::ext

    sqs -->|task_id| wrapper
    wrapper -->|fetch task| dispatcher
    wrapper -->|fetch secrets| secrets
    wrapper -->|inject config + secrets| operator
    wrapper -->|heartbeat| dispatcher
    
    operator -->|read/write| postgres
    operator -->|write| s3
    operator -->|publish buffered records| buffers
    operator -.->|platform jobs only| rpc
    
    wrapper -->|report status| dispatcher
    wrapper -->|ack| sqs

    classDef component fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
    classDef ext fill:#eee,stroke:#666,color:#000;
```

## Runtime Model

| Runtime | Execution | Use Case |
|---------|-----------|----------|
| `dispatcher` | In-process | Platform-only jobs |
| `lambda` | AWS Lambda | Cron/webhook/manual sources, lightweight reactive operators |
| `ecs_rust` | ECS (Rust) | Ingest, transforms, compaction |
| `ecs_python` | ECS (Python) | ML, pandas |

**Queue model (ECS):** One SQS queue per runtime. SQS payload includes `task_id` only; the worker wrapper fetches `operator`, `config`, and event context (`cursor`/`partition_key`) from the Dispatcher.

**Lambda sources:** Invoked by EventBridge/API Gateway, emit upstream events to Dispatcher.

**Lambda reactive jobs:** Invoked by Dispatcher when upstream datasets update (jobs with `runtime: lambda`). Dispatcher invokes the Lambda with the **full task payload** (same shape as `/internal/task-fetch`) and does not wait; a task is “done” only when the Lambda reports `/internal/task-complete`. Timeouts/crashes are handled by the reaper + retries (`max_attempts`) and Lambda built-in retries should be disabled (Dispatcher owns retries uniformly).

**ECS:** Long-polls SQS, stays warm per `idle_timeout`, heartbeats to Dispatcher.

**Architecture (v1):** ECS worker images run on `linux/amd64` to keep user bundle targeting simple. Additional architectures (e.g., `arm64`) can be introduced as separate runtimes in the registry.

## Related

- [contracts.md](../contracts.md) — worker/dispatcher contract
- [dispatcher.md](dispatcher.md) — orchestration and backpressure
- [udf.md](../../features/udf.md) — sandbox model (for user code)
- [security_model.md](../../standards/security_model.md) — isolation model

