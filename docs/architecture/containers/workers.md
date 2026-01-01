# Workers

Executors. One worker image per runtime.

Trace uses **two worker profiles** with different trust assumptions:

- **Platform workers** run trusted platform operators (block follower, ingest, compaction). They may use platform-managed secrets and may reach the RPC Egress Gateway.
- **UDF workers** run untrusted user code (alerts, custom transforms). They do **not** have direct Postgres access and receive scoped data access via Query Service + Dispatcher credential minting.

## Component View

### Platform Worker

```mermaid
flowchart LR
    subgraph Worker["Platform Worker Task"]
        wrapper["Worker Wrapper"]:::component
        operator["Platform Operator"]:::component
    end

    sqs["SQS task queues"]:::infra
    buffers["SQS dataset buffers"]:::infra
    dispatcher["Dispatcher"]:::component
    postgres["Postgres data"]:::database
    s3["S3 Parquet"]:::database
    rpcgw["RPC Egress Gateway"]:::component

    sqs -->|task_id| wrapper
    wrapper -->|claim task + lease| dispatcher
    wrapper -->|inject config + env| operator
    wrapper -->|heartbeat| dispatcher
    
    operator -->|read/write| postgres
    operator -->|write| s3
    operator -->|publish buffered records| buffers
    operator -.->|platform jobs only| rpcgw
    
    wrapper -->|report status| dispatcher
    wrapper -->|ack| sqs

    classDef component fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
```

> Secrets are injected at task launch (ECS task definition `secrets`) and are available to the operator as environment variables. Platform operators do not fetch Secrets Manager directly at runtime.

### UDF Worker

```mermaid
flowchart LR
    subgraph Worker["UDF Worker Task"]
        wrapper["Worker Wrapper"]:::component
        udf["User Code"]:::component
    end

    sqs["SQS task queues"]:::infra
    buffers["SQS dataset buffers"]:::infra
    dispatcher["Dispatcher"]:::component
    qs["Query Service"]:::component
    broker["Dispatcher credential minting"]:::component
    s3["S3 Parquet"]:::database

    sqs -->|task_id| wrapper
    wrapper -->|claim task + lease| dispatcher
    wrapper -->|inject config + capability token| udf
    wrapper -->|heartbeat| dispatcher

    udf -->|SELECT SQL scoped| qs
    udf -->|exchange token| broker
    broker -->|scoped STS creds| udf
    udf -->|read/write scoped| s3
    udf -->|publish buffered records| buffers

    wrapper -->|report status| dispatcher
    wrapper -->|ack| sqs

    classDef component fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
```

## Runtimes

| Runtime | Execution | Trust | Use Case |
|---------|-----------|-------|----------|
| `dispatcher` | In-process | trusted | Platform-only control-plane jobs |
| `lambda` | AWS Lambda | mixed | Sources (cron/webhook/manual), lightweight reactive operators |
| `ecs_rust` | ECS (Rust) | trusted | Ingest, transforms, compaction |
| `ecs_python` | ECS (Python) | trusted | Platform ML/transforms |
| `ecs_udf_ts` | ECS (Node) | untrusted | User-defined logic (alerts/transforms) |
| `ecs_udf_python` | ECS (Python) | untrusted | User-defined ML/pandas |
| `ecs_udf_rust` | ECS (Rust) | untrusted | User-defined high-perf scanning |

## Execution Model

- **ECS runtimes**: receive `task_id` from SQS, claim the task to obtain `(attempt, lease_token, payload)`, execute, heartbeat, then complete. The wrapper extends SQS visibility for long tasks.
- **Lambda runtimes**:
  - **Sources**: invoked by EventBridge / API Gateway and emit upstream events.
  - **Reactive jobs**: invoked by Dispatcher with the full task payload and must report completion.

Notes:
- v1 targets `linux/amd64` for ECS runtimes; additional architectures can be added as new runtimes.


