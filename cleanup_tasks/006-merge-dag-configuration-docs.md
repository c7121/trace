# Cleanup Task 006: Merge DAG configuration and deployment docs

## Goal
Eliminate overlap between DAG configuration and DAG deployment documentation by making one file the canonical owner.

## Why
There is likely overlap between:
- `docs/specs/dag_configuration.md` (spec-level behavior and configuration semantics)
- `docs/architecture/dag_deployment.md` (deployment workflow and invariants)

When both describe "how DAGs are defined and deployed", duplication is hard to avoid.

## Recommendation
Make `docs/specs/dag_configuration.md` the canonical owner for configuration semantics, and keep `docs/architecture/dag_deployment.md` focused on system-level deployment invariants only (or delete it if it becomes redundant).

## Plan
- Audit both docs and decide one of:
  - Option A (preferred): reduce `docs/architecture/dag_deployment.md` to the minimal system invariants and link to the spec for details.
  - Option B: merge the unique parts of `docs/architecture/dag_deployment.md` into `docs/specs/dag_configuration.md` and delete `docs/architecture/dag_deployment.md`.
- Update links to point to the canonical owner.

## Files to touch
- `docs/specs/dag_configuration.md`
- `docs/architecture/dag_deployment.md`
- Any docs that link to the removed or slimmed file

## Acceptance criteria
- Only one place explains DAG configuration semantics in detail.
- Any remaining DAG deployment doc is short and invariant-focused.
- No broken links.

## Reduction
- Delete or drastically shorten one of the overlapping docs.

## Suggested commit message
`docs: reduce duplication in DAG deployment docs`

