# Dispatcher

Central orchestration coordinator. Primary control-plane service.

> **Note on Postgres:** the docs use “Postgres” as a technology for two separate databases:
> - **Postgres state** — control-plane source of truth (jobs, tasks, versions, leases)
> - **Postgres data** — data-plane hot tables (alerts, hot chain tables, query results, etc.)
>
> They are deployed as **two separate instances/clusters** (e.g., two RDS databases), even if they share the same engine/version.

## Architecture Overview

System-level container relationships are documented in [c4.md](../c4.md).

This document focuses on Dispatcher responsibilities and internal components.

## Responsibilities

**Responsibilities:**
- Route all upstream events to dependent jobs
- Create tasks and enqueue to operator queues (outbox → SQS)
- Handle `runtime: dispatcher` jobs in-process (platform-only)
- Monitor source job health (ECS workers with `activation: source`, `source.kind: always_on`)
- Track in-flight jobs per operator (scaling control)
- Run reaper for dead tasks
- Publish queue depth metrics to CloudWatch
- Expose manual source API (emits events)

## Event Model

Every job emits events when it materializes outputs. At runtime, outputs are identified by a stable `dataset_uuid` plus a **generation** `dataset_version`.

Minimal cursor event:

```json
{"dataset_uuid": "uuid", "dataset_version": "uuid", "cursor": 12345}
```

Partition event (block-range example):

```json
{"dataset_uuid": "uuid", "dataset_version": "uuid", "partition_key": "1000000-1010000", "start": 1000000, "end": 1010000}
```

**Routing rule:** by default, the Dispatcher routes only events for the dataset's **current** `dataset_version` (older generations may be accepted for audit but are not routed).

Events are treated as **at-least-once** and may be duplicated or arrive out of order. Correctness comes from task leasing + idempotent outputs. See [task_lifecycle.md](../task_lifecycle.md).

## Event Routing

**Event routing:**
1. Worker emits event: `{dataset_uuid: "...", dataset_version: "...", cursor: 12345}`
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

Dispatcher is stateless — durable state lives in Postgres state. On failure/restart:

- ECS restarts the service.
- In-flight workers may continue executing their current attempt.
- If a worker cannot heartbeat/report completion during the outage, it retries until the Dispatcher is reachable again.
- Queued tasks are not lost: enqueue intents are persisted via the Postgres state outbox and published after restart.

Because execution is **at-least-once**, a long outage may cause some duplicate work (e.g., leases expire and tasks are retried). Output commits and routing are designed to be idempotent.

Task wake-ups are delivered via SQS task queues. Correctness comes from leasing in Postgres state; see [task_lifecycle.md](../task_lifecycle.md).

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
    task_sqs["SQS task queues"]:::infra
    buffers["SQS dataset buffers"]:::infra
    postgres_state["Postgres state"]:::database
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
