# Backlog

Non-phase-specific items deferred from v1.

## Recommended build order

This document is a recommended build order for Trace that minimizes risk and keeps each milestone independently testable.
It is intentionally high level; the canonical behavior is defined in the architecture docs (C4, contracts, task lifecycle).

### Milestone 0: Foundations

- Networking and IAM skeleton (VPC, private subnets, VPC endpoints as needed)
- RDS **Postgres state** and **Postgres data**
- S3 buckets (datasets, results, scratch)
- SQS queues (task queues, dataset buffers) + DLQs
- CloudWatch log groups and basic dashboards

**Exit criteria:** services can start, connect to dependencies, and emit logs and metrics.

### Milestone 1: Dispatcher core

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

### Milestone 2: Worker wrapper (platform workers)

- ECS service that long-polls SQS task queues
- Claim/heartbeat/complete calls to Dispatcher
- A “no-op” operator that exercises the lifecycle
- Visibility timeout extension while a task is running

**Exit criteria:** worker retries do not create duplicate committed outputs; the system rehydrates after restarts.

### Milestone 3: Dataset commit protocol

- Dataset registry entries and dataset version pointers (Postgres state)
- Replace/append staging layout on S3
- Dispatcher commit rules (attempt fenced)

**Exit criteria:** a failed attempt never becomes visible to readers; a retry produces a single committed output.

### Milestone 4: Buffered dataset sinks

- Dataset buffer schema: `dedupe_key`, payload, destination
- Sink worker that drains SQS dataset buffers and writes to Postgres data using upserts

**Exit criteria:** duplicate buffer messages do not create duplicate rows in Postgres data.

### Milestone 5: Query Service + Dispatcher credential minting

- Query Service:
  - org queries and task-scoped queries
  - export large results to S3
- Capability token issuance in Dispatcher
- Dispatcher credential minting: mint scoped STS credentials for task inputs and outputs

**Exit criteria:** UDF and platform workers can read allowed datasets and write outputs without direct Secrets Manager access.

### Milestone 6: UDF execution and alerting pipeline

- UDF runtime:
  - invoke query service for reads
  - write outputs to S3 staging and complete tasks
- Alert evaluate + route + delivery services using buffered datasets for writes

**Exit criteria:** alerts are end-to-end functional with at-least-once semantics.

### Milestone 7: Production hardening

- Autoscaling policies (workers, query service, sinks)
- Rate limiting and backoff for RPC and delivery
- Operational runbooks:
  - manual replay of DLQs
  - manual GC for committed dataset versions
  - staging cleanup policy

**Exit criteria:** game day drills pass in staging.

### Suggested implementation order

If you want the shortest path to a “useful system”, implement milestones in this order:

1. Milestones 0–2 (durable orchestration)
2. Milestone 4 (sinks) or Milestone 3 (datasets), depending on which use case you value first
3. Milestone 5 (query + broker) to unlock safe UDFs
4. Milestone 6 (alerting) once the platform primitives are stable


## Platform

- User-defined jobs / arbitrary code execution — platform operators first
- Physical tenant isolation — logical isolation sufficient for v1
- Multiple chains — Monad only initially
- Aggregator (fan-in) virtual operator — requires correlation state per partition
- Additional worker runtimes in the registry (e.g., `ecs_r`, `ecs_scala`) are deferred
- Automatic garbage collection policies for committed `dataset_version`s (time-based / count-based) are deferred; v1 uses manual purge (ADR 0009)

## Data Lineage

- Column-level lineage for selective re-materialization — track which columns each job reads from upstream datasets; when only specific columns change, re-process only jobs that depend on those columns (reduces over-processing for wide tables with narrow consumers)

## DAG Configuration

- Schema versioning for forward compatibility
- Rich validation diagnostics (line/field-level errors)
- Environment promotion workflow (dev→staging→prod)

## UDF

- Custom transforms — user logic for reshaping/cleaning data
- Enrichments — add computed fields or external labels

## Alerting

- Per-channel rate limiting / throttling

## Query Service

- Saved queries — save and share queries for reuse
- Discovery — browse available datasets, jobs, assets within org
- Per-org and per-user rate limits

## Visualization

- Dashboard builder — visual representation of query results (charts, tables, maps)
- Job type: `Represent` — following Ben Fry's pipeline taxonomy (Acquire → Parse → Filter → Mine → Represent → Refine → Interact)
- Interactive exploration — user-driven filtering/drill-down on visualized data
- Embedded views — share/embed visualizations externally

## Enterprise Integration Patterns

Patterns for advanced orchestration. See [EIP](https://www.enterpriseintegrationpatterns.com/).

- Wire Tap operator — virtual operator (runtime: dispatcher) that copies events to a secondary destination for debugging/auditing/replay
- Aggregator operator — fan-in for composite triggers (A AND B, N-of-M, timeout); requires correlation state
- Correlation ID — `correlation_id` on tasks for end-to-end tracing across job chains
- Message History — track event path through DAG (`job_path[]` or `event_history` table)
