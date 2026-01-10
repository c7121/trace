# Cleanup Task 019: Rehome operator docs as specs

## Goal
Move operator documentation out of `docs/architecture/` and into `docs/specs/` so the operator surface area is treated as a behavior spec (DAG surface), not as implemented architecture.

## Why
Operator names and config shapes are part of the DAG contract surface. Most operators described today are not implemented yet, so placing them under architecture makes them read as authoritative implementation.

This also reduces confusion for readers and agents: specs answer "what must be true", architecture answers "how the system is structured".

## Plan
- Create `docs/specs/operators/` with:
  - `docs/specs/operators/README.md` (short index)
  - one file per operator moved from `docs/architecture/operators/`
- Add a `Status:` line to each operator spec:
  - `implemented` (and where: harness, Lite, AWS), or
  - `planned`
- Shrink `docs/architecture/operators/README.md` to a short pointer:
  - what an operator is (one paragraph)
  - link to `docs/specs/operators/README.md`
- Update links across `docs/` to point at the new operator spec paths.

## Files to touch
- `docs/specs/operators/` (new)
- `docs/architecture/operators/*` (moved or reduced)
- Any docs that link to operator docs (for example `docs/specs/ingestion.md`)

## Acceptance criteria
- Operator surface area is owned by `docs/specs/operators/`.
- Unimplemented operators are clearly labeled `planned`.
- Architecture no longer implies unimplemented operators exist.
- No information loss: content is moved and linkified, not deleted.

## Suggested commit message
`docs: rehome operator docs under specs`

