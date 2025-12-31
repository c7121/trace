# Dispatcher

Central orchestration coordinator. The only platform service.

## Architecture Overview

Detailed view showing internal structure and data flows.

```mermaid
flowchart LR
    subgraph Trace["Trace Platform"]
        subgraph Orchestration["Orchestration"]
            gateway["Gateway (API/CLI)"]:::container
            dispatcher["Dispatcher"]:::container
        end
        subgraph Execution["Execution"]
            subgraph Queues["Queues (SQS)"]
                task_sqs["Task Queues"]:::infra
                buffers["Dataset Buffers"]:::infra
            end
            ecs_workers["ECS Fargate"]:::container
            lambda["Lambda Functions"]:::container
        end
        subgraph Data["Data"]
            postgres[("Postgres")]:::database
            s3[("S3 (Parquet)")]:::database
            sinks["Dataset Sinks"]:::container
        end
        subgraph TracePlatform["Platform (Trace)"]
            registry["Runtime Registry"]:::infra
            platformAuth["Auth/Policy"]:::infra
            postgres_state[("Postgres (state)")]:::database
            duckdb["DuckDB (Query)"]:::container
            delivery["Delivery Service"]:::container
        end
        subgraph AWSServices["AWS Services"]
            eventbridge["EventBridge (cron)"]:::infra
            platformSec["Secrets Manager"]:::infra
            platformObs["CloudWatch/CloudTrail"]:::infra
            idp["Cognito (IdP)"]:::infra
        end
    end

    users["Users"]:::person
    ops["Platform Ops"]:::person
    rpc["RPC Providers"]:::ext
    webhooks["External Webhooks"]:::ext

    users -->|access| gateway
    ops -->|observe| platformObs
    gateway -->|authn| idp
    gateway -->|request jobs, queries| dispatcher
    gateway -->|queries| duckdb
    
    eventbridge -->|cron| lambda
    gateway -->|webhook| lambda
    dispatcher -->|invoke runtime=lambda| lambda
    lambda -->|emit event| dispatcher
    
    dispatcher -->|create tasks| postgres_state
    dispatcher -->|resolve runtime| registry
    dispatcher -->|enqueue| task_sqs
    task_sqs -->|deliver task| ecs_workers
    
    ecs_workers -->|fetch task, status, heartbeat| dispatcher
    ecs_workers -->|write| postgres
    ecs_workers -->|write| s3
    ecs_workers -->|publish buffered records| buffers
    ecs_workers -->|fetch secrets| platformSec
    ecs_workers -->|fetch chain data| rpc
    ecs_workers -->|emit telemetry| platformObs
    ecs_workers -->|emit upstream event| dispatcher

    delivery -->|poll pending deliveries| postgres
    delivery -->|send notifications| webhooks
    delivery -->|update delivery status| postgres

    lambda -->|write| postgres
    lambda -->|write| s3
    lambda -->|publish buffered records| buffers
    lambda -->|fetch secrets| platformSec

    buffers -->|drain| sinks
    sinks -->|write| postgres
    sinks -->|emit upstream event| dispatcher
    
    duckdb -->|federated query| postgres
    duckdb -->|federated query| s3

    classDef person fill:#f6d6ff,stroke:#6f3fb3,color:#000;
    classDef container fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef ext fill:#eee,stroke:#666,color:#000;
```

## Responsibilities

**Responsibilities:**
- Route all upstream events to dependent jobs
- Create tasks and enqueue to operator queues (SQS)
- Handle `runtime: dispatcher` jobs in-process (platform-only)
- Monitor source job health (ECS workers with `activation: source`, `source.kind: always_on`)
- Track in-flight jobs per operator (scaling control)
- Run reaper for dead tasks
- Publish queue depth metrics to CloudWatch
- Expose manual source API (emits events)

## Event Model

**Event model:**

Every job emits **one event per output** when it materializes data. At runtime, outputs are identified by `dataset_uuid` (a system UUID). User-facing `dataset_name` is resolved via the dataset registry for publishing/querying.

```json
{"dataset_uuid": "uuid", "cursor": 12345}
```

Events can also include partition or row-range context when relevant:

```json
{"dataset_uuid": "uuid", "partition_key": "1000000-1010000"}
```

The Dispatcher routes based on the upstream output identity (`dataset_uuid`). Reactive jobs that list the output as an input edge receive the event.

## Event Routing

**Event routing:**
1. Worker emits event: `{dataset_uuid: "...", cursor: 12345}`
2. Dispatcher queries: jobs whose input edges reference that `dataset_uuid`
3. For each dependent reactive job:
   - If `runtime: dispatcher` → Dispatcher handles directly
   - Else if `runtime: lambda` → create task, invoke Lambda
   - Else → create task, enqueue to SQS

## Backpressure

**Backpressure:**

Propagates upstream through DAG edges. When a queue trips its threshold (depth or age), Dispatcher pauses upstream producers recursively. When pressure clears (depth drops below threshold), Dispatcher unpauses and producers resume.

- Per-job thresholds: `max_queue_depth`, `max_queue_age`
- Mode: `pause` (stop task creation until queue drains)
- Priority tiers: `normal`, `backfill` — shed `backfill` first when under pressure

## Out of Scope

**Does NOT:**
- Execute compute tasks (that's workers)
- Pull from queues
- Evaluate cron schedules (that's EventBridge + Lambda)

## Failure Mode

**Failure mode:**

Dispatcher is stateless — all state lives in Postgres. On failure:
- ECS auto-restarts the service (RTO: ~1 minute)
- Workers continue executing in-flight tasks from SQS
- Source jobs (e.g., `block_follower`) continue running and writing data
- Event routing pauses, but no events are lost — downstream jobs use cursor-based catch-up on restart
- No data loss, only delayed processing

## SQS Queues

Task dispatch mechanism for ECS workers.

**Why SQS over Postgres-as-queue:**
- Efficient long-polling (workers block on SQS, not busy-loop on Postgres)
- Native ECS autoscaling integration
- Built-in visibility timeout
- Workers stay dumb — no orchestration logic

**Configuration:**
- FIFO queue with deduplication
- Visibility timeout: 5 minutes (configurable per job)
- Dead letter queue after 3 failed receives

## Component View

```mermaid
flowchart LR
    subgraph Sources["Lambda Sources"]
        cronSrc["Cron Source"]:::component
        webhookSrc["Webhook Source"]:::component
    end

    subgraph Dispatch["Dispatcher"]
        taskCreate["Task Creator"]:::component
        eventRouter["Upstream Event Router"]:::component
        reaper["Dead Task Reaper"]:::component
        sourceMon["Source Monitor"]:::component
        manualApi["Manual Trigger API"]:::component
    end

    eventbridge["EventBridge"]:::infra
    gateway["Gateway"]:::infra
    task_sqs["SQS Task Queues"]:::infra
    buffers["Dataset Buffers (SQS)"]:::infra
    postgres_state["Postgres (state)"]:::database
    workers["ECS Workers"]:::component
    lambdaOps["Lambda Operators"]:::component
    sinks["Dataset Sink"]:::component

    eventbridge -->|invoke| cronSrc
    gateway -->|invoke| webhookSrc
    
    cronSrc -->|emit event| eventRouter
    webhookSrc -->|emit event| eventRouter
    manualApi -->|create task| taskCreate
    
    workers -.->|upstream event| eventRouter
    lambdaOps -.->|upstream event| eventRouter
    eventRouter -->|find dependents| postgres_state
    eventRouter -->|create tasks| taskCreate
    
    taskCreate -->|create task| postgres_state
    taskCreate -->|enqueue runtime=ecs_*| task_sqs
    taskCreate -->|invoke runtime=lambda| lambdaOps
    reaper -->|check heartbeats| postgres_state
    reaper -->|mark failed| postgres_state
    sourceMon -->|check health| postgres_state

    workers -->|publish records| buffers
    lambdaOps -->|publish records| buffers
    buffers -->|drain| sinks
    sinks -.->|upstream event| eventRouter

    classDef component fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
```

## Runtime Registry (Extensible)

Runtimes are identifiers used by the Dispatcher to select a worker image and queue.
They are modeled as strings (not a fixed enum) to allow future additions.

**Registry responsibilities:**
- Map `runtime` → worker image and SQS queue.
- Declare capabilities (e.g., supports long-running tasks, source jobs, GPU, etc.).
- Define default resource limits and heartbeat expectations.

**Adding a new runtime:**
1. Build a worker image (e.g., `ecs_r` for R).
2. Register it in the Dispatcher config with queue + capabilities.
3. Use `runtime: ecs_r` in job YAML.

## Related

- [contracts.md](../contracts.md) — task, event, and API schemas
- [orchestration.md](../data_model/orchestration.md) — job/task schemas
- [event_flow.md](../event_flow.md) — end-to-end sequence diagram
- [security_model.md](../../standards/security_model.md) — isolation model

