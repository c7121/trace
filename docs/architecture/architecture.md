# ETL Orchestration System Architecture

**Version:** 1.2  
**Date:** December 2025

---

## Table of Contents

1. [Overview](#overview)
2. [Build Plan](#build-plan)
3. [System Architecture](#system-architecture)
4. [Core Components](#core-components)
5. [Data Model](#data-model)
6. [Access Control](#access-control)
7. [PII and User Data](#pii-and-user-data)
8. [Job Lifecycle](#job-lifecycle)
9. [DAG Configuration](#dag-configuration)
10. [Infrastructure](#infrastructure)
11. [Deployment](#deployment)

---

## Overview

A general-purpose ETL orchestration system designed for:

- **Multi-runtime support** — Rust, Python, R, TypeScript, Scala
- **Asset-based lineage** — Everything produces trackable assets
- **Flexible partitioning** — Data-driven, not static time-based
- **Source jobs** — Long-running services with `activation: source` (e.g., blockchain followers)
- **Config-as-code** — DAGs defined in YAML, version controlled

### Design Principles

1. **Everything is a job** — Streaming services, batch transforms, checks
2. **Everything produces assets** — Postgres tables, S3 Parquet, any URI
3. **Workers are dumb** — Receive task, execute, report result
4. **YAML is source of truth** — Definitions in git, state in Postgres
5. **Single dispatcher** — Simple, stateless, restartable

---

## Build Plan

Phased approach: prove orchestration and data flow before user-facing features.

| Phase | Components | Validates |
|-------|------------|-----------|
| 0 | Terraform scaffolding (VPC, ECS, SQS, RDS, S3) | Infrastructure provisioning |
| 1 | Dispatcher + Lambda sources + Worker wrapper | Orchestration layer |
| 2 | `block_follower` → Postgres | Real-time ingestion to hot storage |
| 2 | `cryo_ingest` → S3 (parallel) | Historical backfill to cold storage |
| 3 | Query service + `query` job (hot only) | Query path works |
| 4 | `parquet_compact` | Hot → cold compaction lifecycle |
| 5 | Query service + `query` job (federated) | Query spans hot + cold |
| 6 | `alert_evaluate` + `alert_deliver` | User-facing alerting |
| 7 | `integrity_check` | Cold storage verification |

### Deferred

- User-defined jobs / arbitrary code execution — platform operators first
- Physical tenant isolation — logical isolation sufficient for v1
- Multiple chains — Monad only initially
- Aggregator (fan-in) virtual operator — requires correlation state per partition

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
    subgraph Trace["Trace Platform (VPC)"]
        subgraph Orchestration["Orchestration"]
            gateway["Gateway (API/CLI)"]:::container
            dispatcher["Dispatcher"]:::container
            registry["Runtime Registry"]:::infra
            sqs["SQS Queues"]:::infra
        end
        subgraph Compute["Workers"]
            workers["Workers (ECS Fargate)"]:::container
        end
        subgraph Storage["Storage"]
            postgres["Postgres (hot + state)"]:::database
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

    subgraph Serverless["Serverless"]
        eventbridge["EventBridge"]:::infra
        lambda["Lambda Sources"]:::container
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
    
    dispatcher -->|create tasks| postgres
    dispatcher -->|resolve runtime| registry
    dispatcher -->|enqueue| sqs
    sqs -->|deliver task| workers
    
    workers -->|fetch task, update status| postgres
    workers -->|write hot data| postgres
    workers -->|write cold data| s3
    workers -->|fetch secrets| platformSec
    workers -->|fetch chain data| rpc
    workers -->|deliver alerts| webhooks
    workers -->|emit telemetry| platformObs
    workers -->|emit upstream event| dispatcher
    
    duckdb -->|federated query| postgres
    duckdb -->|federated query| s3

    classDef person fill:#f6d6ff,stroke:#6f3fb3,color:#000;
    classDef container fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef ext fill:#eee,stroke:#666,color:#000;
```

### Event Flow

```mermaid
sequenceDiagram
    participant Src as Source (Lambda/ECS)
    participant D as Dispatcher
    participant PG as Postgres
    participant Q as SQS
    participant W as Worker
    participant S as Storage

    Src->>D: Emit event {dataset, cursor}
    D->>PG: Find jobs where input_datasets matches
    D->>PG: Create tasks
    D->>Q: Enqueue to operator queue
    Q->>W: Deliver task
    W->>S: Execute, write output
    W->>PG: Update task status
    W->>D: Emit event {output_dataset, cursor}
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
    sqs["SQS Queues"]:::infra
    postgres["Postgres"]:::database
    workers["ECS Workers"]:::component

    eventbridge -->|invoke| cronSrc
    gateway -->|invoke| webhookSrc
    
    cronSrc -->|emit event| eventRouter
    webhookSrc -->|emit event| eventRouter
    manualApi -->|create task| taskCreate
    
    workers -.->|upstream event| eventRouter
    eventRouter -->|find dependents| postgres
    eventRouter -->|create tasks| taskCreate
    
    taskCreate -->|create task| postgres
    taskCreate -->|enqueue| sqs
    reaper -->|check heartbeats| postgres
    reaper -->|mark failed| postgres
    sourceMon -->|check health| postgres

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
    postgres["Postgres"]:::database
    s3["S3"]:::database
    secrets["Secrets Manager"]:::infra
    rpc["RPC Providers"]:::ext

    sqs -->|task_id| wrapper
    wrapper -->|fetch task| postgres
    wrapper -->|fetch secrets| secrets
    wrapper -->|inject config + secrets| operator
    wrapper -->|heartbeat| postgres
    
    operator -->|read/write hot| postgres
    operator -->|write cold| s3
    operator -.->|platform jobs only| rpc
    
    wrapper -->|update status| postgres
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

Every job emits an event when it writes to its `output_dataset`. The event is simple:

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

**Does NOT:**
- Execute compute tasks (that's workers)
- Pull from queues
- Evaluate cron schedules (that's EventBridge + Lambda)

### 2. SQS Queues

Task dispatch mechanism. One queue per runtime.

**Why SQS over Postgres-as-queue:**
- Push to workers (no polling loop in workers)
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
- RPC key claims (resource pool)

### 5. Asset Storage

**Hot path:** Postgres
- Immediate writes
- Immediate queries for alerts
- UPDATE/DELETE for reorgs

**Cold path:** S3 Parquet
- Compacted from hot
- Immutable partitions
- Analytics optimized

**Query layer:** DuckDB
- Spans both Postgres and S3
- Federated queries

---

## Data Model

### Entity Relationship

```mermaid
erDiagram
    JOBS ||--o{ TASKS : creates
    TASKS ||--o{ TASK_INPUTS : records
    TASKS }o--|| DATA_PARTITIONS : produces
    ORGS ||--o{ JOBS : owns
    ORGS ||--o{ USERS : contains
    USERS ||--o{ ADDRESS_LABELS : creates
    USERS ||--o{ SAVED_QUERIES : creates
    USERS ||--o{ ALERT_DEFINITIONS : creates
    
    ORGS {
        uuid id PK
        text name UK
        text slug UK
        jsonb settings
        timestamp created_at
    }
    
    USERS {
        uuid id PK
        uuid org_id FK
        text external_id UK
        text email
        text role
        timestamp created_at
    }
    
    JOBS {
        uuid id PK
        uuid org_id FK
        text name UK
        text dag_name
        text activation
        text runtime
        text operator
        jsonb source
        text execution_strategy
        text idle_timeout
        jsonb config
        text config_hash
        text[] input_datasets
        text output_dataset
        jsonb scaling
        int timeout_seconds
        int heartbeat_timeout_seconds
        int max_attempts
        boolean active
        timestamp created_at
        timestamp updated_at
    }
    
    TASKS {
        uuid id PK
        uuid job_id FK
        text status
        text[] partitions
        jsonb input_versions
        text worker_id
        timestamp started_at
        timestamp completed_at
        timestamp last_heartbeat
        int attempts
        timestamp next_retry_at
        text error_message
        text output_path
        bigint output_row_count
        timestamp created_at
    }
    
    TASK_INPUTS {
        uuid task_id PK
        text input_dataset PK
        text input_partition PK
        timestamp input_version
    }
    
    RPC_KEY_CLAIMS {
        int key_index PK
        text worker_id
        timestamp claimed_at
        timestamp last_heartbeat
    }
    
    ADDRESS_LABELS {
        uuid id PK
        uuid org_id FK
        uuid user_id FK
        text address
        text label
        text visibility
        timestamp created_at
        timestamp updated_at
    }
    
    SAVED_QUERIES {
        uuid id PK
        uuid org_id FK
        uuid user_id FK
        text name
        text query
        text visibility
        timestamp created_at
        timestamp updated_at
    }
    
    ALERT_DEFINITIONS {
        uuid id PK
        uuid org_id FK
        uuid user_id FK
        text name
        jsonb condition
        jsonb channels
        text visibility
        boolean enabled
        timestamp created_at
        timestamp updated_at
    }
```

### Schema SQL

```sql
-- Organizations
CREATE TABLE orgs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    settings JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT now()
);

-- Job definitions (synced from YAML)
CREATE TABLE jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    name TEXT NOT NULL,
    dag_name TEXT NOT NULL,
    activation TEXT NOT NULL,           -- 'source', 'reactive'
    runtime TEXT NOT NULL,              -- 'lambda', 'ecs_rust', 'ecs_python', 'dispatcher'
    operator TEXT NOT NULL,             -- 'block_follower', 'alert_evaluate', etc.
    source JSONB,                       -- { "kind": "cron", "schedule": "0 0 * * *" }
    execution_strategy TEXT,            -- NULL for sources, else 'PerUpdate', 'PerPartition', 'Bulk'
    idle_timeout TEXT,                  -- reactive only: 'never', '5m', '0', etc.
    config JSONB NOT NULL DEFAULT '{}', -- operator config
    config_hash TEXT NOT NULL,
    input_datasets TEXT[],
    output_dataset TEXT,
    scaling JSONB,                      -- { "mode": "backfill", "max_concurrency": 20 }
    timeout_seconds INT,
    heartbeat_timeout_seconds INT,
    max_attempts INT DEFAULT 3,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    CONSTRAINT unique_job_name UNIQUE (dag_name, name)
);

-- Task instances (append-only)
CREATE TABLE tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES jobs(id),
    status TEXT NOT NULL DEFAULT 'Queued',
    partitions TEXT[],
    input_versions JSONB,
    worker_id TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    last_heartbeat TIMESTAMPTZ,
    attempts INT DEFAULT 0,
    next_retry_at TIMESTAMPTZ,
    error_message TEXT,
    output_path TEXT,
    output_row_count BIGINT,
    created_at TIMESTAMPTZ DEFAULT now()
);

-- Task input versions (for memoization)
CREATE TABLE task_inputs (
    task_id UUID REFERENCES tasks(id),
    input_dataset TEXT NOT NULL,
    input_partition TEXT NOT NULL,
    input_version TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (task_id, input_dataset, input_partition)
);

-- RPC key pool
CREATE TABLE rpc_key_claims (
    key_index INT PRIMARY KEY,
    worker_id TEXT,
    claimed_at TIMESTAMPTZ,
    last_heartbeat TIMESTAMPTZ
);

-- Column lineage (optional)
CREATE TABLE column_lineage (
    output_dataset TEXT NOT NULL,
    output_column TEXT NOT NULL,
    input_dataset TEXT NOT NULL,
    input_column TEXT NOT NULL,
    job_id UUID REFERENCES jobs(id),
    PRIMARY KEY (output_dataset, output_column, input_dataset, input_column)
);

-- Indexes
CREATE INDEX idx_tasks_status ON tasks(status) WHERE status IN ('Queued', 'Running');
CREATE INDEX idx_tasks_job_id ON tasks(job_id);
CREATE INDEX idx_tasks_next_retry ON tasks(next_retry_at) WHERE status = 'Failed';
CREATE INDEX idx_tasks_last_heartbeat ON tasks(last_heartbeat) WHERE status = 'Running';
CREATE INDEX idx_jobs_active ON jobs(dag_name) WHERE active = true;

-- PII: Address labels
CREATE TABLE address_labels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    user_id UUID NOT NULL REFERENCES users(id),
    address TEXT NOT NULL,
    label TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'private',  -- private, org, public
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (org_id, user_id, address, label)
);

-- PII: Saved queries
CREATE TABLE saved_queries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    query TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'private',
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- PII: Alert definitions
CREATE TABLE alert_definitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    condition JSONB NOT NULL,
    channels JSONB NOT NULL,  -- email, sms, webhook configs
    visibility TEXT NOT NULL DEFAULT 'private',
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- PII access audit log
CREATE TABLE pii_access_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    table_name TEXT NOT NULL,
    record_id UUID NOT NULL,
    action TEXT NOT NULL,  -- read, write, delete
    accessed_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_address_labels_org ON address_labels(org_id);
CREATE INDEX idx_address_labels_user ON address_labels(user_id);
CREATE INDEX idx_address_labels_address ON address_labels(address);
CREATE INDEX idx_saved_queries_org ON saved_queries(org_id);
CREATE INDEX idx_alert_definitions_org ON alert_definitions(org_id);
CREATE INDEX idx_pii_access_log_user ON pii_access_log(user_id);
CREATE INDEX idx_pii_access_log_time ON pii_access_log(accessed_at);
```

---

## Access Control

**Hierarchy:** Global → Org → Role (reader/writer/admin) → User

**Identity:** Users authenticate via external IdP (OIDC/SAML). `external_id` links to IdP subject.

**Enforcement:** All API requests include org context. Jobs, tasks, assets scoped by `org_id`.

**Tenant isolation:** Logical by default (queries filtered by `org_id`). Physical isolation optional (separate Terraform deployment).

---

## PII and User Data

PII = address labels, saved queries, alert definitions.

All PII tables have: `org_id`, `user_id`, `visibility` (private/org/public).

**Rules:**
- `private`: only creator can read
- `org`: any org member can read
- Reads logged to `pii_access_log`
- Hard delete only (GDPR)

---

## Job Lifecycle

### Job Properties

Every job has two key properties:

| Property | Description | Values |
|----------|-------------|--------|
| `activation` | How the job is started | `source`, `reactive` |
| `runtime` | Where code executes | `lambda`, `ecs_rust`, `ecs_python`, `dispatcher` |

Reactive jobs can also set:

| Property | Description | Values |
|----------|-------------|--------|
| `idle_timeout` | How long to stay alive with no work | `never`, duration (`5m`), or `0` |

**Activation behavior:**

| Activation | Behavior | Example |
|------------|----------|--------|
| `source` | Emits events; not scheduled by Dispatcher | `block_follower` (always_on) |
| `reactive` | Runs from Dispatcher tasks | `alert_evaluate` — reacts to new blocks |

**Source kinds:** `always_on`, `cron`, `webhook`, `manual`.

**Cron and manual:** They are source kinds that emit events. Reactive jobs subscribe
to `input_datasets` and are scheduled by the Dispatcher.

### Execution Strategies

How work is divided for reactive jobs:

| Strategy | Description | Use Case |
|----------|-------------|----------|
| `PerUpdate` | One task per upstream event | Alert evaluation |
| `PerPartition` | One task per partition | Historical backfills |
| `Bulk` | Single task for all pending work | Compaction, aggregations |

Source jobs do not have an execution strategy — they emit events directly.

### Task States

```mermaid
stateDiagram-v2
    [*] --> Queued: Task created
    Queued --> Running: Worker picks up
    Running --> Completed: Success
    Running --> Failed: Error / Timeout
    Failed --> Queued: Retry (if attempts < max)
    Failed --> [*]: Max retries exceeded
    Completed --> [*]
```

### Source Job Lifecycle (activation: source)

```mermaid
stateDiagram-v2
    [*] --> Pending: Job deployed
    Pending --> Running: Dispatcher ensures running
    Running --> Running: Heartbeat OK
    Running --> Dead: Heartbeat timeout
    Dead --> Pending: Dispatcher restarts
    Running --> Draining: Shutdown signal
    Draining --> [*]: Graceful stop
```

### Reactive Job Lifecycle (activation: reactive)

```mermaid
stateDiagram-v2
    [*] --> Idle: No events
    Idle --> Running: Event received
    Running --> Running: More events
    Running --> Idle: idle_timeout exceeded
    Idle --> [*]: Scale to zero (optional)
```

### Staleness, Memoization & Reorgs

See [data_versioning.md](data_versioning.md) for full specification of:
- Partition vs. cursor-based incremental processing
- Staleness detection and memoization
- Reorg handling and invalidations
- Alert deduplication

### Scaling

Dispatcher-controlled, not AWS auto-scaling.

| Mode | Behavior |
|------|----------|
| `backfill` | Max parallelism (e.g., 20 concurrent) |
| `steady` | Single worker |

```yaml
scaling:
  mode: backfill
  max_concurrency: 20
```

Dispatcher tracks in-flight jobs and only releases work when slots available.

---

## DAG Configuration

### Directory Structure

```
/dags
  /monad
    dag.yaml
  /ethereum
    dag.yaml
  /ml-pipeline
    dag.yaml
```

### YAML Schema

```yaml
name: monad

defaults:
  heartbeat_timeout_seconds: 60
  max_attempts: 3

jobs:
  # Source: Lambda cron emits daily event
  - name: daily_trigger
    activation: source
    runtime: lambda
    operator: cron_source
    source:
      kind: cron
      schedule: "0 0 * * *"
    output_dataset: daily_events

  # Source: always-running block follower
  - name: block_follower
    activation: source
    runtime: ecs_rust
    operator: block_follower
    source:
      kind: always_on
    config:
      chain_id: 10143
      rpc_pool: monad
    output_dataset: hot_blocks
    heartbeat_timeout_seconds: 60

  # Source: manual backfill requests
  - name: backfill_request
    activation: source
    runtime: lambda
    operator: manual_source
    source:
      kind: manual
    output_dataset: backfill_requests
    
  # Reactive: evaluate alerts on new blocks
  - name: alert_evaluate
    activation: reactive
    runtime: ecs_rust
    operator: alert_evaluate
    execution_strategy: PerUpdate
    idle_timeout: 5m
    input_datasets: [hot_blocks]
    output_dataset: triggered_alerts
    timeout_seconds: 60
    
  # Batch: compact triggered by daily cron
  - name: compact_blocks
    activation: reactive
    runtime: ecs_rust
    operator: parquet_compact
    execution_strategy: Bulk
    idle_timeout: 0
    input_datasets: [hot_blocks, daily_events]
    output_dataset: cold_blocks
    timeout_seconds: 1800
    
  # Backfill: manual source emits partitioned backfill requests
  - name: cryo_backfill
    activation: reactive
    runtime: ecs_rust
    operator: cryo_ingest
    execution_strategy: PerPartition
    idle_timeout: 0
    input_datasets: [backfill_requests]
    config:
      chain_id: 10143
      datasets: [blocks, transactions, logs]
    scaling:
      mode: backfill
      max_concurrency: 20
    output_dataset: cold_blocks
    timeout_seconds: 3600
```

### Deploy Process

```mermaid
flowchart LR
    YAML[dag.yaml] --> PARSE[Parse YAML]
    PARSE --> VALIDATE[Validate Schema]
    VALIDATE --> DEACTIVATE[Deactivate existing jobs]
    DEACTIVATE --> UPSERT[Upsert by name]
    UPSERT --> ACTIVATE[Set active=true]
    ACTIVATE --> PROVISION[Provision Lambda/EventBridge for sources]
```

**SQL logic:**
```sql
UPDATE jobs SET active = false WHERE dag_name = 'monad';

INSERT INTO jobs (name, dag_name, activation, runtime, operator, source, execution_strategy, idle_timeout, ...)
VALUES ('block_follower', 'monad', 'source', 'ecs_rust', 'block_follower', '{"kind":"always_on"}', NULL, NULL, ...)
ON CONFLICT (dag_name, name) DO UPDATE SET
  activation = EXCLUDED.activation,
  runtime = EXCLUDED.runtime,
  operator = EXCLUDED.operator,
  source = EXCLUDED.source,
  execution_strategy = EXCLUDED.execution_strategy,
  idle_timeout = EXCLUDED.idle_timeout,
  config = EXCLUDED.config,
  config_hash = EXCLUDED.config_hash,
  active = true,
  updated_at = now();
```

---

## Infrastructure

### AWS Architecture

```mermaid
flowchart TB
    subgraph VPC["VPC"]
        subgraph Public["Public Subnets"]
            ALB[Application Load Balancer]
            APIGW[API Gateway]
        end
        
        subgraph Private["Private Subnets"]
            subgraph ECS["ECS Cluster"]
                DISPATCHER_SVC[Dispatcher Service]
                POLARS_WORKERS[Polars Workers]
                PYTHON_WORKERS[Python Workers]
                INGEST_WORKERS[Ingest Workers]
            end
            
            RDS[(RDS Postgres)]
        end
    end
    
    subgraph Serverless["Serverless"]
        EVENTBRIDGE[EventBridge Rules]
        LAMBDA[Lambda Sources]
    end
    
    subgraph AWS_Services["AWS Services"]
        SQS_QUEUES[SQS Queues]
        S3_BUCKET[S3 Data Bucket]
        ECR[ECR Repositories]
        CW[CloudWatch]
        SM[Secrets Manager]
    end
    
    EVENTBRIDGE --> LAMBDA
    APIGW --> LAMBDA
    LAMBDA --> DISPATCHER_SVC
    
    ALB --> DISPATCHER_SVC
    
    DISPATCHER_SVC --> RDS
    DISPATCHER_SVC --> SQS_QUEUES
    DISPATCHER_SVC --> CW
    
    SQS_QUEUES --> POLARS_WORKERS
    SQS_QUEUES --> PYTHON_WORKERS
    SQS_QUEUES --> INGEST_WORKERS
    
    POLARS_WORKERS --> RDS
    POLARS_WORKERS --> S3_BUCKET
    PYTHON_WORKERS --> RDS
    PYTHON_WORKERS --> S3_BUCKET
    INGEST_WORKERS --> RDS
    
    POLARS_WORKERS --> SM
    INGEST_WORKERS --> SM
    
    ECR --> ECS
```

### Terraform Structure

```
/terraform
  /modules
    /vpc           # VPC, subnets, NAT, VPC endpoints
    /rds           # Postgres, security groups
    /ecs           # Cluster, services, task definitions, autoscaling
    /sqs           # FIFO queues, DLQ
    /s3            # Data bucket, lifecycle rules
    /lambda        # Lambda sources, API Gateway
    /eventbridge   # Cron schedules
  /environments
    /dev
    /prod
```

Key resources per module:
- **VPC**: Private/public subnets, VPC endpoints for S3/SQS/Secrets Manager
- **ECS**: Fargate services, SQS-based autoscaling
- **RDS**: Postgres 15, encrypted, multi-AZ in prod
- **SQS**: FIFO with deduplication, 5min visibility, DLQ after 3 failures
- **S3**: Versioned, lifecycle to Glacier after 1 year

---

## Deployment

**Order:**
1. Terraform apply (infra)
2. Database migrations
3. Sync DAG YAML → Postgres
4. Deploy ECS services

**Rollback:** Terraform state rollback, ECS deployment rollback, git revert DAGs.

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

---

## Appendix

### Glossary

| Term | Definition |
|------|------------|
| Operator | Job implementation (e.g., `block_follower`, `alert_evaluate`) |
| Operator Type | Runtime category: `lambda`, `polars`, `python`, `ingest` |
| Activation | `source` (emits events) or `reactive` (runs from tasks) |
| Source | Job with `activation: source` — maintains connections, emits events |
| Asset | Output of a job — Parquet file, table rows |
| Partition | A subset of an asset (e.g., blocks 0-10000) |

### References

- [cryo GitHub](https://github.com/paradigmxyz/cryo)
- [DuckDB Documentation](https://duckdb.org/docs/)
- [AWS ECS Autoscaling](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/service-auto-scaling.html)
