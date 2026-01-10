# Cleanup Task 012: Docs navigation and workflow

## Goal
Make the docs easier to navigate for humans and predictable for agents by clarifying:
- what each directory is for, and
- a design, implement, validate workflow with the canonical docs to consult.

## Stance
- Specs are feature and behavior surfaces (JTBD and externally observable semantics).
- Architecture is the stable system model: invariants, contracts, trust boundaries, and C4 views.
- Wire-level interface contracts should be centralized (so payload shapes and invariants do not drift across many docs).
- Operator docs that define DAG surface area (operator name, config semantics, inputs/outputs) belong under `docs/specs/` (for example `docs/specs/operators/`).
- Deploy docs are AWS-oriented; Trace Lite procedures belong in examples.

## Plan
- Update `docs/README.md` to add a short "Workflow" section:
  - Design: read the relevant spec, then confirm invariants and contracts.
  - Implement: follow container docs and the relevant operator spec; update data model docs if schemas change.
  - Validate: run the harness gates and use examples for end-to-end verification.
- Add cross-links so C4 is easy to find but not duplicated:
  - Link from `docs/deploy/infrastructure.md` to `docs/architecture/c4.md`.
  - Link from `docs/architecture/c4.md` to `docs/deploy/infrastructure.md` as the AWS deployment view.

## Files to touch
- `docs/README.md`
- `docs/architecture/c4.md`
- `docs/deploy/infrastructure.md`

## Acceptance criteria
- A human can answer "where do I start?" in under a minute.
- An agent has a clear reading and update order without adding new policy docs.
- No duplication of C4 diagrams across architecture and deploy docs.

## Suggested commit message
`docs: clarify workflow and docs navigation`
