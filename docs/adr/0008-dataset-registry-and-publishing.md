# ADR 0008: Dataset Registry + Publishing

## Status
- Accepted (December 2025)

## Decision

- The system distinguishes:
  - `dataset_name`: human-readable, user-defined string (unique per org) used for discovery and most user-facing APIs
  - `dataset_uuid`: system-generated UUID primary key used internally and in storage paths
- `dataset_name` format is constrained in v1:
  - max length: 128
  - regex: `^[a-z][a-z0-9_]{0,127}$` (lower `snake_case`)
- DAG YAML wiring is by **job output indices**, not dataset names:
  - internal edges connect `{job, output_index} -> {job, input_index}`
  - internal edges do not require user-defined dataset naming
- A top-level `publish:` section registers a specific `{job, output_index}` as a user-visible dataset:
  - Publishing is **metadata-only** (registry update/aliasing)
  - Publishing does **not** change execution/bootstrap/rematerialization behavior
- The dataset registry links published datasets back to their producer (`dag_name`, `job_name`, `output_index`) for navigation and “single producer” enforcement.
  - Exception: some published datasets are **buffered sink datasets** (e.g., `alert_events`) intended to be **multi-writer** within a DAG; producer provenance is tracked per record (e.g., `producer_job_id`) rather than by a single owning job output.
- A dataset’s materialization lifecycle is owned by its producing DAG. Other DAGs can read/subscribe (shared reads) but do not “drive” the producer dataset’s materialization.
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
  - `dataset_name` format (v1 regex + max length)
  - producer uniqueness per dataset (single producer DAG/job/output), except for explicitly-declared buffered sink datasets that allow multi-writer
- Internal platform APIs and event routing use `dataset_uuid`.
- If a user needs to query/debug an internal edge, they must explicitly publish it (or publish a snapshot in the future).

## v1 Policy

- Publish entries name datasets and attach access policy/metadata, but **do not** declare full storage schema/backends inline. Storage schema is determined by the producing job/output and recorded by the platform.
- Snapshot publishing is deferred (future work). v1 publishes **live** dataset pointers; retention of committed versions follows ADR 0009 (manual GC).

## Related

- Normative surface: [dag_configuration.md](../specs/dag_configuration.md) and [user_api_contracts.md](../architecture/user_api_contracts.md)
- Orchestration schema mapping: [orchestration.md](../architecture/data_model/orchestration.md)
- Query Service dataset discovery: [query_service_user_query.md](../specs/query_service_user_query.md)
- Versioning and pinning: [ADR 0009](0009-atomic-cutover-and-query-pinning.md)
