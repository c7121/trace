# Cleanup Task 016: Move operator recipes to examples

## Goal
Reduce section sprawl in operator docs by moving the end-to-end recipe content into `docs/examples/`.

## Why
Operator docs should be small, contract-like references: inputs, outputs, idempotency, and dependencies.

The current recipe sections add a lot of headings and narrative content inside operator docs. This makes the operator catalog harder to scan and increases drift risk.

## Plan
- Extract the recipe sections into dedicated example docs under `docs/examples/`:
  - Chain liveliness monitoring
  - RPC integrity checking
  - Validator monitoring
- Replace each in-operator recipe section with a short link to the new example doc.
- Update `docs/specs/operators/README.md` to link to the new example docs.

## Files to touch
- `docs/specs/operators/liveliness_monitor.md`
- `docs/specs/operators/rpc_integrity_check.md`
- `docs/specs/operators/validator_stats.md`
- `docs/specs/operators/README.md`
- Add new example docs under `docs/examples/`

## Acceptance criteria
- Operator docs no longer contain `## Recipe:` sections.
- The operator catalog still exposes recipes via links.
- No information is lost; recipe content is moved, not deleted.

## Suggested commit message
`docs: move operator recipes to examples`
