# Metadata and lineage

Status: Draft
Owner: Platform
Last updated: 2026-01-02

## Summary
Trace records lineage and run metadata for jobs, tasks, datasets, partitions, and dataset versions. This metadata supports discovery, debugging, rollback/query pinning, and operational visibility.

## Risk
Low

## Problem statement
Users and operators need to answer:
- what produced this dataset version?
- what inputs did it use?
- what ran, when, and why did it fail?
without scraping logs or reconstructing state from S3.

Constraints:
- The platform is at-least-once; metadata must be resilient to duplicate task completion.
- We maintain a hard boundary between Postgres **state** (control-plane) and Postgres **data** (data-plane). Cross-DB referential integrity is application-enforced.

## Goals
- Provide a consistent metadata model for:
  - datasets and dataset versions,
  - partitions/materializations,
  - task execution history,
  - operator-provided custom metadata.
- Support query pinning and atomic cutover for S3-backed datasets.

## Non-goals
- A full graph query language in v1.
- User-defined metadata schemas beyond JSON.

## Public surface changes
- Query surfaces: dataset registry + version pinning behaviors (see ADRs).
- Persistence: metadata tables in Postgres state and (where applicable) Postgres data.

## Architecture (C4) - Mermaid-in-Markdown only

```mermaid
flowchart LR
  EXEC[Executors] -->|task completion + metadata| DISP[Dispatcher]
  DISP -->|persist| PS[(Postgres state)]
  QS[Query Service] -->|reads| PS
  QS -->|reads data| PD[(Postgres data)] & S3[S3]
```

## Proposed design

### What the system tracks
At minimum:
- **Organizations and users** (identity and ownership; see security model for auth).
- **DAG versions** and the active DAG mapping.
- **Jobs and tasks** including attempt history, leases, and failure reasons.
- **Datasets** (registry mapping from name → UUID).
- **Dataset versions** and published pointers (atomic cutover, query pinning).
- **Partitions/materializations** for datasets that are partitioned.
- **Custom metadata** (JSON) emitted by operators and stored with materializations.

### Versioning and rollback semantics
- Dataset registry and publishing: `docs/adr/0008-dataset-registry-and-publishing.md`
- Atomic cutover and query pinning: `docs/adr/0009-atomic-cutover-and-query-pinning.md`
- Partition versioning and invalidations: `docs/architecture/data_versioning.md`

V1 constraint (be explicit):
- Atomic cutover/rollback semantics apply to **version-addressed outputs** (e.g., S3 manifests + pointers).
- Postgres “hot” tables are treated as live mutable state unless versioned views/tables are implemented.

### Data model references
Canonical DDL lives in:
- `docs/architecture/data_model/orchestration.md`
- `docs/architecture/data_model/data_versioning.md`
- `docs/architecture/data_model/address_labels.md`
- `docs/architecture/data_model/pii.md`

## Contract requirements
- Dispatcher MUST be the system of record for task lifecycle and dataset version commits.
- Task completion MUST be idempotent: duplicate completions MUST NOT create duplicate materializations/versions.
- Cross-DB identifiers (`org_id`, `job_id`, `task_id`) stored in Postgres data MUST be treated as soft refs; trusted writers validate them at write time (see `docs/architecture/db_boundaries.md`).

## Security considerations
- Metadata is multi-tenant and may contain sensitive derived fields. Apply the same authz rules as data access:
  - user JWT auth for user-facing queries,
  - capability tokens for task-scoped queries,
  - no direct Postgres access for untrusted runtimes.

## Alternatives considered
- Store lineage only in S3 manifests.
  - Why not: hard to query and join with runtime/task history; pushes complexity into every consumer.

## Acceptance criteria
- Tests:
  - dataset publish creates a discoverable registry entry with stable UUID mapping.
  - task completion produces exactly one materialization record per idempotency key.
- Observable behavior:
  - Operators can locate the job/task that produced a dataset version and its inputs.
