# Cleanup Task 013: Specs index and JTBD framing

## Goal
Make `docs/specs/` easier to browse by adding a small index and explicitly framing specs as JTBD and behavior surfaces.

## Why
Specs are the primary place to understand "what we are building" and "what must be true". Today there is no single entrypoint under `docs/specs/` so readers rely on search.

## Plan
- Add `docs/specs/README.md` with:
  - a 1-paragraph definition of what a spec is in this repo,
  - a short list of key specs grouped by domain (query, ingestion, chain sync, UDF, security gates),
  - links to architecture docs for invariants and contracts.
- Update `docs/README.md` to link to `docs/specs/README.md` (not just the directory).

## Files to touch
- `docs/specs/README.md`
- `docs/README.md`

## Acceptance criteria
- A human can find the relevant spec without using search.
- The index stays short (links only, no duplicated narrative).

## Suggested commit message
`docs: add specs index`
