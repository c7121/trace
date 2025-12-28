# Proposed Simplifications: Triggering + Query Execution

## Constraints
- Org/User/PII is required (keep multi-tenant tables, access control, audit logging).

## Triggering Model (Simplified)
### Core idea
- Triggering only does two things: (1) route events by dataset name, (2) create tasks
  for reactive jobs.
- Everything else is a source activation detail, not a trigger type.

### Two activation categories
1. Source
   - Emits events; not scheduled by Dispatcher.
   - Variants:
     - always_on (ECS)
     - cron (EventBridge -> Lambda)
     - webhook (API Gateway -> Lambda)
     - manual (API emits event)
   - Output event shape: `{dataset, cursor, metadata?}`
2. Reactive job
   - Runs from Dispatcher task.
   - Declares `inputs` and `execution_strategy`.

### YAML sketch
```yaml
jobs:
  - name: block_follower
    activation: source
    source:
      kind: always_on
    output_dataset: hot_blocks

  - name: daily_trigger
    activation: source
    source:
      kind: cron
      schedule: "0 0 * * *"
    output_dataset: daily_events

  - name: alert_evaluate
    activation: reactive
    execution_strategy: per_update
    inputs: [hot_blocks]
    output_dataset: triggered_alerts
```

### Lambda clarity
- Lambda is a source implementation (cron/webhook/manual).
- If you want Lambda as a job runtime, treat it as `runtime: lambda` inside a
  reactive job, but the trigger semantics stay the same.

### Remove/rename
- Remove trigger values like `cron`, `manual`, `none`, `upstream`.
- Replace with `activation: source|reactive` and `source.kind` when applicable.

## Query Model: One Feature, Two Execution Modes
### Goal
- Keep queries as jobs, but allow a simple REST endpoint for small queries.

### Proposed model
- One "query" capability with two execution modes:
  - interactive: REST API runs query directly (tight limits).
  - batch: API creates a query job (same operator) and returns job_id.

### API behavior
- POST /v1/query
  - if query fits limits, execute inline and return rows or presigned URL.
  - else (or if `mode: batch`), enqueue query_job.

### Operator alignment
- One operator: `query` (DuckDB).
- Mode is a request parameter, not a separate operator.

### YAML sketch
```yaml
- name: daily_summary
  activation: reactive
  operator: query
  execution_strategy: bulk
  config:
    query: |
      SELECT date_trunc('day', block_timestamp) as day,
             count(*) as tx_count
      FROM unified_transactions
      GROUP BY 1
    output_format: parquet
```

## Doc impact checklist (no edits yet)
- Update `docs/architecture/architecture.md` trigger terminology and DAG schema.
- Update `docs/architecture/operators/README.md` to reflect activation vs runtime.
- Collapse `docs/architecture/services/query_service.md` and
  `docs/architecture/operators/duckdb_query.md` into a single "Query" doc with
  dual modes.
