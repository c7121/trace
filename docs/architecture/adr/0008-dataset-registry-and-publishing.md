# ADR 0008: Dataset Registry + Publishing

## Status
- Accepted (December 2025)

## Decision

- The system distinguishes:
  - `dataset_name`: human-readable, user-defined string (unique per org) used for discovery and most user-facing APIs
  - `dataset_uuid`: system-generated UUID primary key used internally and in storage paths
- DAG YAML wiring is by **job output indices**, not dataset names:
  - internal edges connect `{job, output_index} -> {job, input_index}`
  - internal edges do not require user-defined dataset naming
- A top-level `publish:` section registers a specific `{job, output_index}` as a user-visible dataset:
  - Publishing is **metadata-only** (registry update/aliasing)
  - Publishing does **not** change execution/backfill/rematerialization behavior
- The dataset registry links published datasets back to their producer (`dag_name`, `job_name`, `output_index`) for navigation and “single producer” enforcement.
- Query Service exposes only **published** datasets (via registry), not every internal edge.
- Backlog: support **snapshot publishes** (pinned/immutable aliases) for “read as-of”.

## Context

- Requiring a user-defined dataset name for every internal edge is cumbersome and makes DAG edits harder to reason about.
- Users still need human-readable dataset names to navigate and query outputs.
- Storage backends impose naming/escaping constraints; using UUIDs for physical identifiers avoids brittle escaping rules.

## Why

- **UX**: users name only the datasets they want to query/share.
- **Stability**: internal routing uses UUIDs; renames of `dataset_name` do not affect storage identity.
- **Security**: registry becomes the policy enforcement point (e.g., `read_roles`), independent from DAG YAML.
- **Cross-DAG reads**: published datasets provide a clean seam for sharing across DAGs without shared writes.

## Consequences

- Deploy must validate:
  - `dataset_name` uniqueness per org
  - producer uniqueness per dataset (single producer DAG/job/output)
- Internal platform APIs and event routing use `dataset_uuid`.
- If a user needs to query/debug an internal edge, they must explicitly publish it (or publish a snapshot in the future).

## Open Questions

- `dataset_name` normalization rules (case, allowed characters) and whether names are mutable/renamable.
- Whether publish entries can declare full storage schema inline (v1) vs requiring admin registry configuration.
- Snapshot publishes: UX + retention/GC.

