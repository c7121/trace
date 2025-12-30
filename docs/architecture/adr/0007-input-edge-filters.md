# ADR 0007: Input Edge Filters (Read-Time Predicates)

## Status
- Accepted (December 2025)

## Decision
- Filtering is expressed as an annotation on the **input edge** (an `inputs` entry), not as a standalone DAG node.
- `inputs` supports a long form:
  - `inputs: [{ from: { dataset: dataset_a } }]` (no filter)
  - `inputs: [{ from: { dataset: dataset_a }, where: "..." }]` (filtered)
- Dispatcher routes by the upstream output identity (internally `dataset_uuid`); filters are applied by the consumer at read time.
- Cursor semantics are unchanged: on each upstream event, the consumer advances its cursor for the input edge **even if the filter matches zero rows**.

## Why
- Avoids materializing intermediate “filtered datasets” solely for routing.
- Keeps the Dispatcher simple (no query planning or predicate routing).
- Makes routing explicit in DAG YAML (e.g., `critical → pagerduty`, `info/warning → slack`).

## `where` Predicate Rules (v1)
- Must be a **pure boolean predicate** (safe to append to a `WHERE` clause).
- Allowed: `AND`/`OR`/`NOT`, parentheses, comparisons, `IN (...)` with literals, `IS NULL`, `LIKE`.
- Disallowed: subqueries (`SELECT`, `EXISTS`, `IN (SELECT ...)`), statement separators (`;`), DDL/DML keywords, and non-deterministic functions (`now()`, `random()`, etc.).

## Consequences
- DAG validation must lint `where` and reject unsupported constructs.
- Task details include the per-input `where` so operators can apply SQL pushdown (Postgres) or query-engine filtering (DuckDB).
- When filters need reuse/audit/backpressure as a first-class signal, introduce a real intermediate dataset instead of an edge filter.
