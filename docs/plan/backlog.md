# Backlog

Non-phase-specific items deferred from v1.

## Platform

- Untrusted `ecs_udf` execution (v2) once a zero-trust isolation/credential story exists.
- Multi-tenant (shared infra) and/or physical tenant isolation (per-org deployments).
- Multiple chains beyond the initial target.
- Automatic garbage collection policies for committed dataset versions (v1 uses manual purge; see ADR 0009).

## Orchestration

- Aggregator/fan-in virtual operator (requires correlation state).
- Rich bounded recompute UX beyond the minimal API (once defined).

## Data lineage

- Column-level lineage for selective re-materialization.

## DAG configuration

- Schema versioning for forward compatibility.
- Rich validation diagnostics (line/field-level errors).
- Environment promotion workflow (dev → staging → prod).

## UDF

- Custom transforms and enrichments beyond alert evaluation.

## Alerting

- Per-channel rate limiting/throttling beyond coarse delivery retry policies.

## Query Service

- Saved queries and sharing.
- Dataset discovery UX.
- Fine-grained per-org/per-user rate limits.

## Visualization

- Dashboard builder and embedded views.

## Enterprise Integration Patterns

Patterns that require additional state/complexity:

- Wire Tap operator (copy events to a secondary destination for debugging/auditing).
- Aggregator/correlation operator (A AND B, N-of-M, timeout).
- Message history tracking (`job_path[]` / event history tables).
