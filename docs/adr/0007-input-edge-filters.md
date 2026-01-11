# ADR 0007: Input Edge Filters (Read-Time Filters)

## Status
- Accepted (December 2025)

### Amendment (January 2026)
- `where` is a **structured filter map** (not an arbitrary SQL predicate string). This keeps the DAG YAML safely validatable and avoids Postgres/DuckDB dialect drift.

## Decision
- Filtering is expressed as an annotation on the **input edge** (an `inputs` entry), not as a standalone DAG node.
- `inputs` supports a long form:
  - `inputs: [{ from: { dataset: dataset_a } }]` (no filter)
  - `inputs: [{ from: { dataset: dataset_a }, where: { ... } }]` (filtered)
- Dispatcher routes by the upstream output identity (internally `dataset_uuid`); filters are applied by the consumer at read time.
- Cursor semantics are unchanged: on each upstream event, the consumer advances its cursor for the input edge **even if the filter matches zero rows**.

## Why
- Avoids materializing intermediate “filtered datasets” solely for routing.
- Keeps the Dispatcher simple (no query planning or predicate routing).
- Makes routing explicit in DAG YAML (e.g., `critical → pagerduty`, `info/warning → slack`).

## `where` filter rules (v1)
- `where` is a **map** of field → value.
- Semantics are **AND** across keys.
- Values are one of:
  - scalar equality (`severity: "critical"`), or
  - list membership (`severity: ["warning", "critical"]`), which is equivalent to `IN`.

Constraints:
- No arbitrary SQL strings.
- Only allowlisted fields are permitted for each referenced dataset/operator.
- Values must type-check (e.g., `chain_id` is an integer, `severity` is an enum).

Example:

```yaml
inputs:
  - from: { dataset: alert_events }
    where:
      severity: critical
      chain_id: 1
```

## Consequences
- DAG validation must lint `where` and reject unsupported keys/types.
- Task payloads include the structured `where` per input edge so operators can apply safe pushdown:
  - Postgres: parameterized `WHERE` clauses,
  - DuckDB: typed predicate filters.
- When filters need reuse/audit/backpressure as a first-class signal, introduce a real intermediate dataset instead of an edge filter.

## Related

- Normative surface: [dag_configuration.md](../specs/dag_configuration.md)
- Example usage: [alert_route.md](../specs/operators/alert_route.md)
- Task payload shape: [task_scoped_endpoints.md](../architecture/contracts/task_scoped_endpoints.md)
