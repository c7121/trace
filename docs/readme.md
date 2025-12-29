# ETL Orchestration System Architecture

**Version:** 1.0.0  
**Date:** December 2025

---

## Table of Contents

1. [Overview](#overview)
2. [System Architecture](#system-architecture)
3. [Core Components](#core-components)
4. [Data Model](#data-model)
5. [Access Control](#access-control)
6. [PII and User Data](#pii-and-user-data)
7. [Job Lifecycle](#job-lifecycle)
8. [DAG Configuration](#dag-configuration)
9. [Infrastructure](#infrastructure)
10. [Deployment](#deployment)

---

## Overview

A general-purpose ETL orchestration system designed for:

- **Multi-runtime support** — Rust, Python, R, TypeScript, Scala
- **Asset-based lineage** — Everything produces trackable assets
- **Flexible partitioning** — Data-driven, not static time-based
- **Source jobs** — Long-running services with `activation: source` (e.g., blockchain followers)
- **Config-as-code** — DAGs defined in YAML, version controlled

See the Build Plan in [docs/plan/build.md](../plan/build.md) for the phased delivery roadmap.

### Design Principles

1. **Everything is a job** — Streaming services, batch transforms, checks
2. **Everything produces assets** — Postgres tables, S3 Parquet, any URI
3. **Workers are dumb** — Receive task, execute, report result
4. **YAML is source of truth** — Definitions in git, state in Postgres
5. **Single dispatcher** — Simple, stateless, restartable

### Tenancy Model

> **v1 is single-tenant.** The architecture includes `org_id` scoping throughout (jobs, tasks, data, queries) to support future multi-tenant expansion, but v1 deploys as a single-org instance. Multi-tenancy (shared infrastructure with logical isolation) and physical tenant isolation (per-org deployments) are deferred. See [backlog.md](../plan/backlog.md).

### Job Characteristics

- **Containerized**: jobs run as containers or services, called remotely (not co-located)
- **Polyglot**: any runtime — Rust, Python, TypeScript, etc. — packaged as a container
- **Standard contract**: jobs receive inputs, produce outputs, return metadata
- **Composable**: jobs can depend on outputs of other jobs, forming DAGs

### Job Types

| Type | Purpose | Example |
|------|---------|---------|
| Ingest | Pull data from onchain or offchain sources | `block_follower`, `cryo_ingest` |
| Transform | Alter, clean, reshape data | decode logs |
| Combine | Join or merge datasets | onchain + offchain |
| Enrich | Add labels, annotations, computed fields | address tagging |
| Summarize | Aggregate, roll up, compute metrics | daily volumes |
| Validate | Check invariants, data quality | `integrity_check` |
| Alert | Evaluate conditions, deliver notifications | `alert_evaluate`, `alert_deliver` |

### Glossary

| Term | Definition |
|------|------------|
| Operator | Job implementation (e.g., `block_follower`, `alert_evaluate`) |
| Activation | `source` (emits events) or `reactive` (runs from tasks) |
| Source | Job with `activation: source` — maintains connections, emits events |
| Asset | Output of a job — Parquet file, table rows |
| Partition | A subset of an asset (e.g., blocks 0-10000) |
| Runtime | Execution environment: `lambda`, `ecs_rust`, `ecs_python`, `dispatcher` |

---

## System Architecture

### System Context

```mermaid
flowchart TB
    users["Users (Analysts / Researchers / Ops)"]:::person
    ops["Platform Ops"]:::person
    idp["IdP (Cognito/SSO)"]:::ext
    trace["Trace Platform"]:::system
    rpc["RPC Providers"]:::ext
    obs["Observability - CloudWatch/CloudTrail"]:::ext
    webhooks["External Webhooks"]:::ext

    users -->|query, configure jobs, alerts| trace
    trace -->|authn/authz| idp
    trace -->|logs/metrics/audit| obs
    ops -->|observe/manage| obs
    trace -->|read chain data| rpc
    trace -->|deliver alerts| webhooks

    classDef person fill:#f6d6ff,stroke:#6f3fb3,color:#000;
    classDef system fill:#d6f6ff,stroke:#1f6fa3,color:#000;
    classDef ext fill:#eee,stroke:#666,color:#000;
```

### Container View

```mermaid
flowchart LR
    subgraph Trace["Trace Platform"]
        subgraph Orchestration["Orchestration"]
            gateway["Gateway (API/CLI)"]:::container
            dispatcher["Dispatcher"]:::container
            registry["Runtime Registry"]:::infra
            task_sqs["SQS Task Queues"]:::infra
            buffers["Dataset Buffers (SQS)"]:::infra
        end
        subgraph Compute["Workers (VPC)"]
            workers["Workers (ECS Fargate)"]:::container
            sinks["Dataset Sink (ECS)"]:::container
        end
        subgraph Serverless["Serverless"]
            eventbridge["EventBridge"]:::infra
            lambda["Lambda Sources"]:::container
        end
        subgraph Storage["Storage"]
            postgres_hot["Postgres (hot data)"]:::database
            postgres_state["Postgres (state)"]:::database
            s3["S3 (Parquet cold)"]:::database
        end
        subgraph Query["Query"]
            duckdb["DuckDB"]:::container
        end
        subgraph Platform["Platform Services"]
            platformAuth["Auth/Policy"]:::infra
            platformSec["Secrets Manager"]:::infra
            platformObs["CloudWatch/CloudTrail"]:::infra
        end
    end

    users["Users"]:::person
    ops["Platform Ops"]:::person
    idp["IdP (Cognito/SSO)"]:::ext
    rpc["RPC Providers"]:::ext
    webhooks["External Webhooks"]:::ext

    users -->|access| gateway
    ops -->|observe| platformObs
    gateway -->|authn| idp
    gateway -->|request jobs, queries| dispatcher
    gateway -->|queries| duckdb
    
    eventbridge -->|cron| lambda
    gateway -->|webhook| lambda
    lambda -->|emit event| dispatcher
    
    dispatcher -->|create tasks| postgres_state
    dispatcher -->|resolve runtime| registry
    dispatcher -->|enqueue| task_sqs
    task_sqs -->|deliver task| workers
    
    workers -->|fetch task, status, heartbeat| dispatcher
    workers -->|write hot data| postgres_hot
    workers -->|write cold data| s3
    workers -->|publish buffered records| buffers
    workers -->|fetch secrets| platformSec
    workers -->|fetch chain data| rpc
    workers -->|deliver alerts| webhooks
    workers -->|emit telemetry| platformObs
    workers -->|emit upstream event| dispatcher

    buffers -->|drain| sinks
    sinks -->|write hot data| postgres_hot
    sinks -->|emit upstream event| dispatcher
    
    duckdb -->|federated query| postgres_hot
    duckdb -->|federated query| s3

    classDef person fill:#f6d6ff,stroke:#6f3fb3,color:#000;
    classDef container fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef ext fill:#eee,stroke:#666,color:#000;
```

Storage split: Postgres (state) for orchestration metadata (multi-AZ, PITR); Postgres (hot) for recent mutable data (partitioned with retention); S3 for cold Parquet.

### Event Flow

```mermaid
sequenceDiagram
    participant Src as Source (Lambda/ECS)
    participant D as Dispatcher
    participant PG as Postgres (state)
    participant Q as SQS
    participant W as Worker
    participant Sink as Dataset Sink
    participant S as Storage

    Src->>D: Emit event {dataset, cursor}
    D->>PG: Find jobs where input_datasets matches
    D->>PG: Create tasks
    D->>Q: Enqueue to operator queue
    Q->>W: Deliver task
    W->>D: Fetch task details
    W->>S: Execute, write output (S3/Postgres)
    W->>D: Report status + emit event(s) {dataset, cursor|partition_key}

    Note over W,Sink: For buffered Postgres datasets (ADR 0006)\nworkers publish records to an SQS buffer.
    W->>Sink: Publish records (buffer)
    Sink->>S: Write Postgres table
    Sink->>D: Emit event(s) after commit
```

**Flow:** Source emits → Dispatcher routes → Worker executes → Worker emits → repeat.

### Component View: Orchestration

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
    sinks["Dataset Sink"]:::component

    eventbridge -->|invoke| cronSrc
    gateway -->|invoke| webhookSrc
    
    cronSrc -->|emit event| eventRouter
    webhookSrc -->|emit event| eventRouter
    manualApi -->|create task| taskCreate
    
    workers -.->|upstream event| eventRouter
    eventRouter -->|find dependents| postgres_state
    eventRouter -->|create tasks| taskCreate
    
    taskCreate -->|create task| postgres_state
    taskCreate -->|enqueue| task_sqs
    reaper -->|check heartbeats| postgres_state
    reaper -->|mark failed| postgres_state
    sourceMon -->|check health| postgres_state

    workers -->|publish records| buffers
    buffers -->|drain| sinks
    sinks -.->|upstream event| eventRouter

    classDef component fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
```

### Component View: Workers

```mermaid
flowchart LR
    subgraph Worker["Worker Container"]
        wrapper["Worker Wrapper"]:::component
        operator["Operator (job code)"]:::component
    end

    sqs["SQS"]:::infra
    buffers["Dataset Buffers (SQS)"]:::infra
    dispatcher["Dispatcher"]:::component
    postgres_hot["Postgres (hot)"]:::database
    s3["S3"]:::database
    secrets["Secrets Manager"]:::infra
    rpc["RPC Providers"]:::ext

    sqs -->|task_id| wrapper
    wrapper -->|fetch task| dispatcher
    wrapper -->|fetch secrets| secrets
    wrapper -->|inject config + secrets| operator
    wrapper -->|heartbeat| dispatcher
    
    operator -->|read/write hot| postgres_hot
    operator -->|write cold| s3
    operator -->|publish buffered records| buffers
    operator -.->|platform jobs only| rpc
    
    wrapper -->|report status| dispatcher
    wrapper -->|ack| sqs

    classDef component fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
    classDef ext fill:#eee,stroke:#666,color:#000;
```

### Component View: Query Service

```mermaid
flowchart LR
    gateway["Gateway"]:::container
    duckdb["DuckDB"]:::component
    postgres["Postgres (hot)"]:::database
    s3["S3 Parquet (cold)"]:::database

    gateway -->|SQL query| duckdb
    duckdb -->|recent data| postgres
    duckdb -->|historical data| s3

    classDef component fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef container fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
```

---

## Core Components

### Platform Components

These are infrastructure services, not jobs:

### 1. Dispatcher

Central orchestration coordinator. The only platform service.

**Responsibilities:**
- Route all upstream events to dependent jobs
- Create tasks and enqueue to operator queues (SQS)
- Handle virtual operators (e.g., `aggregator`) directly — no worker needed
- Monitor source job health (ECS workers with `activation: source`, `source.kind: always_on`)
- Track in-flight jobs per operator (scaling control)
- Run reaper for dead tasks
- Publish queue depth metrics to CloudWatch
- Expose manual source API (emits events)

**Event model:**

Every job emits **one event per output dataset** when it materializes data. The event is simple:

```json
{"dataset": "hot_blocks", "cursor": 12345}
```

Events can also include partition or row-range context when relevant:

```json
{"dataset": "cold_blocks", "partition_key": "1000000-1010000"}
```

The Dispatcher routes based on dataset name alone. Reactive jobs that list the
dataset as an input receive the event.

**Event routing:**
1. Worker emits event: `{dataset: "hot_blocks", cursor: 12345}`
2. Dispatcher queries: jobs where `input_datasets` includes `"hot_blocks"`
3. For each dependent reactive job:
   - If `runtime: dispatcher` → Dispatcher handles directly
   - Else → create task, enqueue to SQS

**Backpressure:**

Propagates upstream through DAG edges. When a queue trips its threshold (depth or age), Dispatcher pauses upstream producers recursively. When pressure clears (depth drops below threshold), Dispatcher unpauses and producers resume.

- Per-job thresholds: `max_queue_depth`, `max_queue_age`
- Mode: `pause` (stop task creation until queue drains)
- Priority tiers: `normal`, `backfill` — shed `backfill` first when under pressure

**Does NOT:**
- Execute compute tasks (that's workers)
- Pull from queues
- Evaluate cron schedules (that's EventBridge + Lambda)

**Failure mode:**

Dispatcher is stateless — all state lives in Postgres. On failure:
- ECS auto-restarts the service (RTO: ~1 minute)
- Workers continue executing in-flight tasks from SQS
- Source jobs (e.g., `block_follower`) continue running and writing data
- Event routing pauses, but no events are lost — downstream jobs use cursor-based catch-up on restart
- No data loss, only delayed processing

### 2. SQS Queues

Task dispatch mechanism. One queue per runtime.

**Why SQS over Postgres-as-queue:**
- Efficient long-polling (workers block on SQS, not busy-loop on Postgres)
- Native ECS autoscaling integration
- Built-in visibility timeout
- Workers stay dumb — no orchestration logic

**Configuration:**
- FIFO queue with deduplication
- Visibility timeout: 5 minutes (configurable per job)
- Dead letter queue after 3 failed receives

### 3. Workers

Executors. One worker image per runtime.

| Runtime | Execution | Use Case |
|---------|-----------|----------|
| `dispatcher` | In-process | Virtual operators (aggregator, wire_tap) |
| `lambda` | AWS Lambda | Cron/webhook/manual sources |
| `ecs_rust` | ECS (Rust) | Ingest, transforms, compaction |
| `ecs_python` | ECS (Python) | ML, pandas |

**Queue model:** One SQS queue per runtime (except `dispatcher`). Task payload includes
`operator`, `config`, and event context (`cursor` or `partition_key`).

**Lambda:** Invoked by EventBridge/API Gateway, emits event to Dispatcher.

**ECS:** Long-polls SQS, stays warm per `idle_timeout`, heartbeats to Dispatcher.

**Architecture (v1):** ECS worker images run on `linux/amd64` to keep user bundle targeting simple. Additional architectures (e.g., `arm64`) can be introduced as separate runtimes in the registry.

### Runtime Registry (Extensible)

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

### 4. Postgres

Source of truth for all state.

**Stores:**
- Job definitions (synced from YAML)
- Tasks (append-only history)
- Task inputs (for memoization)
- Data partitions (asset registry)

### 5. Asset Storage

**Flexibility:** Jobs can write anywhere (S3, Postgres, external), provided downstream jobs can access the output as input.

**Hot path:** Postgres
- Immediate writes
- Immediate queries for alerts
- UPDATE/DELETE for reorgs

**Cold path:** S3 Parquet
- Compacted from hot
- Immutable partitions
- Analytics optimized

**Manifests:** Emitted per job run for integrity verification.

**Query layer:** DuckDB
- Spans both Postgres and S3
- Federated queries

---

## Data Model

See [erd.md](erd.md) for the complete entity relationship diagram with all fields.

### Schema Reference

Full DDL for all tables:

- **Orchestration** (orgs, users, jobs, tasks, task_inputs, column_lineage): [capabilities/orchestration.md](../capabilities/orchestration.md)
- **PII** (pii_access_log): [capabilities/pii.md](../capabilities/pii.md)
- **Alerting** (alert_definitions, alert_events, alert_deliveries): [capabilities/alerting.md](../capabilities/alerting.md)
- **Data Versioning** (partition_versions, dataset_cursors, data_invalidations): [data_versioning.md](data_versioning.md)
- **Query Service** (saved_queries, query_results): [query_service.md](query_service.md)
- **Operators** (address_labels): [operators/address_labels.md](operators/address_labels.md)

---

## Access Control

**Hierarchy:** Global → Org → Permission Role (reader/writer/admin) → User

**Org Roles:** User-defined roles used for visibility scoping (e.g., `role:finance`). See [orchestration.md](../capabilities/orchestration.md) and [pii.md](../capabilities/pii.md).

**Identity:** Users authenticate via external IdP (OIDC/SAML). `external_id` links to IdP subject.

**Enforcement:** All actions (job execution, data access, config changes) require authn/authz. All API requests include org context. Jobs, tasks, assets scoped by `org_id`.

**Tenancy (v1):** Single-tenant deployment. One org per instance. The data model includes `org_id` on all entities to enable future multi-tenant expansion without schema changes.

**Tenancy (future):** Logical multi-tenancy (shared infra, `org_id` filtering) and physical isolation (per-org Terraform deployment) are supported by the schema but not implemented in v1. See [backlog.md](../plan/backlog.md).

**Cross-org sharing (future):** Users can be granted access to another org's data via explicit grants, not shared infrastructure.

---

## PII and User Data

PII is a column-level classification with visibility controls and audit logging. See [capabilities/pii.md](../capabilities/pii.md) for visibility semantics (including org-defined roles) and `pii_access_log`.

---

## Job Lifecycle

Jobs are defined in DAG YAML and synced into Postgres. The Dispatcher creates task instances for reactive jobs when upstream datasets update; source jobs run continuously and emit upstream events.

- Job fields and configuration: [dag_configuration.md](../capabilities/dag_configuration.md)
- Task lifecycle, retries, heartbeats: [orchestration.md](../capabilities/orchestration.md#task-lifecycle)
- Incremental processing, staleness, reorg invalidations: [data_versioning.md](data_versioning.md)
- Operator contract (task input/output + emit event): [operators/README.md](operators/README.md)

---

## DAG Configuration

See [dag_configuration.md](../capabilities/dag_configuration.md) for:
- YAML schema with examples

See [dag_deployment.md](dag_deployment.md) for:
- Deploy/sync flow and upsert semantics
- Source provisioning

---

## Infrastructure

See [infrastructure.md](../capabilities/infrastructure.md) for:
- AWS architecture diagram
- Terraform module structure
- Deployment order and rollback

---

## Deployment

Deployment is separated into:
- **Infrastructure**: provision AWS resources via Terraform (VPC, ECS, SQS, RDS, S3).
- **Database**: apply migrations before starting services.
- **DAG sync**: parse/validate DAG YAML and upsert jobs into Postgres (see [dag_deployment.md](dag_deployment.md)).
- **Services**: roll out Dispatcher, workers, Lambda sources, and Query Service.

---

## Monitoring

**Key alerts:**
- Queue depth > 1000
- Task failure rate > 5%
- Source heartbeat stale > 2 min
- Workers at max capacity

**Logging:** Structured JSON to CloudWatch, 30 day retention.

---

## Security

**IAM roles:** dispatcher-role (SQS, RDS, CloudWatch), worker-role (SQS, RDS, S3, Secrets Manager)

**Secrets:** RPC keys and DB creds in Secrets Manager, injected as env vars.

**Network:** Workers in private subnets. VPC endpoints for S3, SQS, Secrets Manager. ALB HTTPS only.

See [security.md](../standards/security.md) for job isolation, threat model, and credential handling.

---

## Appendix

### References

- [cryo GitHub](https://github.com/paradigmxyz/cryo)
- [DuckDB Documentation](https://duckdb.org/docs/)
- [AWS ECS Autoscaling](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/service-auto-scaling.html)
