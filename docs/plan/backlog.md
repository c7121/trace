# Backlog

Non-phase-specific items deferred from v1.

## Platform

- User-defined jobs / arbitrary code execution — platform operators first
- Physical tenant isolation — logical isolation sufficient for v1
- Multiple chains — Monad only initially
- Aggregator (fan-in) virtual operator — requires correlation state per partition
- Additional worker runtimes in the registry (e.g., `ecs_r`, `ecs_scala`) are deferred

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
