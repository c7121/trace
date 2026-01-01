# Workers

Executors. Two worker profiles (platform and UDF).

Trace uses **two worker profiles** with different trust assumptions:

- **Platform workers** run trusted platform operators (block follower, ingest, compaction). They may use platform-managed secrets and may reach the RPC Egress Gateway.
- **UDF workers** run untrusted user code (alerts, custom transforms). They do **not** have direct Postgres access and receive scoped data access via Query Service + short-lived credentials minted by the Dispatcher.

## Component View

### Platform Worker

```mermaid
flowchart LR
    subgraph Worker["Platform Worker Task"]
        wrapper["Worker Wrapper"]:::component
        operator["Platform Operator"]:::component
    end

    queue["Task queue"]:::infra
    bufferq["Buffer queue"]:::infra
    dispatcher["Dispatcher"]:::component
    postgres["Postgres data"]:::database
    s3["S3 Parquet"]:::database
    rpcgw["RPC Egress Gateway"]:::component

    queue -->|task_id wake-up| wrapper
    wrapper -->|claim task + lease| dispatcher
    wrapper -->|inject config + env| operator
    wrapper -->|heartbeat| dispatcher
    
    operator -->|read/write| postgres
    operator -->|write| s3
    operator -->|publish buffered records| bufferq
    operator -.->|platform jobs only| rpcgw
    
    wrapper -->|report status| dispatcher
    wrapper -->|ack| queue

    classDef component fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
```

> Secrets are injected at task launch (ECS task definition `secrets`) and are available to the operator as environment variables. Platform operators do not fetch Secrets Manager directly at runtime.
>
> Internal Dispatcher endpoints (`/internal/*`) are protected by mTLS. Only the **wrapper** container receives the client certificate; untrusted code must not.

### UDF Worker

```mermaid
flowchart LR
    subgraph Worker["UDF Worker Task"]
        wrapper["Worker Wrapper"]:::component
        udf["User Code"]:::component
    end

    queue["Task queue"]:::infra
    bufferq["Buffer queue"]:::infra
    dispatcher["Dispatcher"]:::component
    qs["Query Service"]:::component
    s3["S3 Parquet"]:::database

    queue -->|task_id wake-up| wrapper
    wrapper -->|claim task + lease| dispatcher
    wrapper -->|inject config + capability token| udf
    wrapper -->|heartbeat| dispatcher

    udf -->|SELECT SQL scoped| qs
    udf -->|request temp creds| dispatcher
    dispatcher -->|scoped STS creds| udf
    udf -->|read/write scoped| s3
    udf -->|publish buffered records| bufferq

    wrapper -->|report status| dispatcher
    wrapper -->|ack| queue

    classDef component fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
```

## Runtimes

To reduce operational surface area, v1 treats language/runtime packaging as an implementation detail and exposes only **four** runtime categories:

| Runtime | Execution | Trust | Use Case |
|---------|-----------|-------|----------|
| `dispatcher` | In-process | trusted | Control-plane-only jobs |
| `lambda` | AWS Lambda | trusted | Sources (cron/webhook/manual) and small operators |
| `ecs_platform` | ECS task | trusted | Platform operators (ingest, compaction, integrity) |
| `ecs_udf` | ECS task | untrusted | User-defined logic (alerts, transforms, queries) |

Notes:
- The operator implementation may be Rust, Python, or Node — that is a build/deployment detail, not a user-facing runtime enum.
- `ecs_udf` is always treated as untrusted and must use Query Service + scoped object-store credentials.

## Execution Model

- **ECS workers**: receive a `task_id` wake-up from the queue backend (SQS in AWS, pgqueue in Lite), claim the task to obtain `(attempt, lease_token, payload)`, execute, heartbeat, then complete. The wrapper extends queue visibility for long tasks.
- **Lambda runtimes**:
  - **Sources**: invoked by EventBridge / API Gateway and emit upstream events.
  - **Reactive jobs**: invoked by Dispatcher with the full task payload and must report completion.

Notes:
- v1 targets `linux/amd64` for ECS workers; additional architectures can be added as needed.


