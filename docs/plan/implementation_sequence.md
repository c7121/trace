# Implementation Sequence

This document is a recommended build order for Trace that minimizes risk and keeps each milestone independently testable.
It is intentionally high level; the canonical behavior is defined in the architecture docs (C4, contracts, task lifecycle).

## Milestone 0: Foundations

- Networking and IAM skeleton (VPC, private subnets, VPC endpoints as needed)
- RDS **Postgres state** and **Postgres data**
- S3 buckets (datasets, results, scratch)
- SQS queues (task queues, dataset buffers) + DLQs
- CloudWatch log groups and basic dashboards

**Exit criteria:** services can start, connect to dependencies, and emit logs and metrics.

## Milestone 1: Dispatcher core

Implement the minimum control-plane required to create and durably track tasks.

- Postgres state schema: jobs, tasks, leases, retries, outbox
- Dispatcher endpoints:
  - create tasks
  - claim task lease
  - heartbeat lease
  - complete attempt (success or failure)
- Outbox worker that publishes:
  - task wake-ups to SQS
  - internal events to EventBridge (if used)

**Exit criteria:** you can enqueue a task into Postgres state and see it wake a worker through SQS.

## Milestone 2: Worker wrapper (platform workers)

- ECS service that long-polls SQS task queues
- Claim/heartbeat/complete calls to Dispatcher
- A “no-op” operator that exercises the lifecycle
- Visibility timeout extension while a task is running

**Exit criteria:** worker retries do not create duplicate committed outputs; the system rehydrates after restarts.

## Milestone 3: Dataset commit protocol

- Dataset registry entries and dataset version pointers (Postgres state)
- Replace/append staging layout on S3
- Dispatcher commit rules (attempt fenced)

**Exit criteria:** a failed attempt never becomes visible to readers; a retry produces a single committed output.

## Milestone 4: Buffered dataset sinks

- Dataset buffer schema: `dedupe_key`, payload, destination
- Sink worker that drains SQS dataset buffers and writes to Postgres data using upserts

**Exit criteria:** duplicate buffer messages do not create duplicate rows in Postgres data.

## Milestone 5: Query Service + Credential Broker

- Query Service:
  - org queries and task-scoped queries
  - export large results to S3
- Capability token issuance in Dispatcher
- Credential Broker: mint scoped STS credentials for task inputs and outputs

**Exit criteria:** UDF and platform workers can read allowed datasets and write outputs without direct Secrets Manager access.

## Milestone 6: UDF execution and alerting pipeline

- UDF runtime:
  - invoke query service for reads
  - write outputs to S3 staging and complete tasks
- Alert evaluate + route + delivery services using buffered datasets for writes

**Exit criteria:** alerts are end-to-end functional with at-least-once semantics.

## Milestone 7: Production hardening

- Autoscaling policies (workers, query service, sinks)
- Rate limiting and backoff for RPC and delivery
- Operational runbooks:
  - manual replay of DLQs
  - manual GC for committed dataset versions
  - staging cleanup policy

**Exit criteria:** game day drills pass in staging.

## Suggested implementation order

If you want the shortest path to a “useful system”, implement milestones in this order:

1. Milestones 0–2 (durable orchestration)
2. Milestone 4 (sinks) or Milestone 3 (datasets), depending on which use case you value first
3. Milestone 5 (query + broker) to unlock safe UDFs
4. Milestone 6 (alerting) once the platform primitives are stable

