# Backlog

Non-phase-specific items deferred from v1.

Rules:
- Backlog items should link to an owning doc, or be promoted into a spec or ADR when they become actionable.
- Avoid restating contracts here - link to the canonical owners.

## Platform

- Untrusted `ecs_udf` execution (v2) once a zero-trust isolation/credential story exists.
- Multi-tenant (shared infra) and/or physical tenant isolation (per-org deployments).
- Multiple chains beyond the initial target.
- Cryo as a library + custom writer/output abstraction: embed Cryo crates in the worker and stream Parquet to the configured object store (S3/MinIO) without local staging.
  - Why: local staging is acceptable for Lite, but production wants fewer moving parts and less disk/cleanup risk for large ranges.
  - Reality check: Cryo writes to local `output_dir` today (no native object-store output), so streaming requires an adapter layer (or upstream Cryo changes).
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
