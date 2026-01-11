# Cleanup Task 049: Tighten DAG configuration spec

## Goal

Make `docs/specs/dag_configuration.md` a stable, low-ambiguity contract for the DAG YAML surface:
- schema-first (clear required vs optional fields),
- examples are self-contained and use documented operators, and
- terminology matches `docs/architecture/README.md` core concepts.

## Why

This spec is close, but a few details can mislead implementers and reviewers:
- The Concepts section defines `Task` in a way that conflates it with `Attempt`.
- The example YAML references fields and operators that are not defined elsewhere (`priority`, `trigger_cron`) and references an undefined job (`block_range_aggregate`).
- Several real YAML fields used in operator examples (`idle_timeout`, `heartbeat_timeout_seconds`, `max_attempts`) are missing from the job field reference table.
- `publish` subfields appear to overlap or conflict with ADR 0008's "publish is metadata-only" policy unless the v1 supported subset is made explicit.

## Assessment summary (from review task 029)

### Clarity issues

- **Concept mismatch:** `Task` is described as a single execution attempt, but Trace core concepts treat `Attempt` as the retry counter and `Task` as the durable unit that can be retried.
- **Example drift:** the top-level example YAML:
  - uses `defaults.priority` which is not defined elsewhere,
  - uses `operator: trigger_cron` which has no operator spec,
  - references `block_range_aggregate` without defining it.
- **Schema coverage gaps:** the job field table omits commonly used fields and creates ambiguity about what is supported vs reserved.

### Duplication and boundaries

- This spec correctly points to `docs/architecture/dag_deployment.md` for deploy/cutover semantics.
- The UDF section overlaps with `docs/specs/operators/udf.md`; DAG config spec should remain the YAML contract and link out for operator behavior details.

## Plan

- Fix Concepts section:
  - Align `Task` and add `Attempt` to match `docs/architecture/README.md`.
  - Remove or clearly mark any non-v1 activation/runtime options (avoid listing unsupported options without an explicit "reserved and rejected" rule).
- Make schema reference explicit:
  - Add a small top-level schema table (`name`, `defaults`, `jobs`, `publish`) with required/optional notes.
  - Expand the Job fields table to include fields that appear in operator docs and examples:
    - `idle_timeout`
    - `heartbeat_timeout_seconds`
    - `max_attempts`
  - Either define `priority` (if intended) or remove it entirely (and do not show it in examples).
- Replace the example YAML with a self-contained example that uses documented operators:
  - `block_follower` (source, `source.kind: always_on`)
  - `range_aggregator` (reactive PerUpdate)
  - `parquet_compact` (reactive PerPartition)
  - Keep `publish` but avoid implying extra publish subfields unless they are v1-supported.
- Clarify publish fields vs ADRs:
  - Add an explicit statement of the v1 supported `publish` surface (metadata-only vs schema declaration).
  - If `publish.*.schema` or `publish.*.write_mode` are planned but not implemented, move them under a "Reserved fields" section and say deploy validation rejects them for v1.
  - Link to ADR 0008 and ADR 0006 for the intended direction.
- Keep operator-specific semantics link-first:
  - For UDF and other operator-specific config semantics, link to the relevant operator spec and avoid restating behavior beyond YAML contract requirements.

## Files to touch

- `docs/specs/dag_configuration.md`

## Acceptance criteria

- The DAG config spec uses terminology consistent with `docs/architecture/README.md`.
- Example YAML is self-contained (no undefined jobs) and only uses documented operators and fields.
- The Job fields table includes all commonly used YAML fields found in operator specs.
- `publish` surface is unambiguous for v1 and does not conflict with ADR guidance.

## Suggested commit message

`docs: tighten dag configuration spec`

