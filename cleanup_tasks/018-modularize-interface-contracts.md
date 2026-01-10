# Cleanup Task 018: Modularize interface contracts

## Goal
Reduce drift and section sprawl by reorganizing wire-level interface contracts into a small set of focused docs, with a single index entrypoint.

## Why
Today, "contract" statements exist in many places (invariants, security, lifecycle, operator docs, and `contracts.md`). Some of that is appropriate, but wire-level payload shapes and endpoint rules should have a single canonical home or they will drift.

This is also a readability issue: `docs/architecture/contracts.md` is long and section-heavy.

## Plan
- Create `docs/architecture/contracts/` containing a small set of focused contract docs, for example:
  - `task_tokens.md` (capability token and verifier rules)
  - `task_endpoints.md` (`/v1/task/*` payload shapes and fencing rules)
  - `events.md` (upstream event schema and partition or cursor semantics)
  - `buffered_datasets.md` (`/v1/task/buffer-publish` and sink contract)
  - `credentials.md` (`/v1/task/credentials` scope derivation rules)
- Reduce `docs/architecture/contracts.md` to an index that links to the focused docs and states scope.
- Update cross-links across `docs/` so other docs link to the specific contract doc instead of re-stating payloads.

## Files to touch
- `docs/architecture/contracts.md`
- Add new docs under `docs/architecture/contracts/`
- Update links in any docs that reference moved sections

## Acceptance criteria
- Payload shapes and endpoint rules are no longer duplicated across multiple docs.
- `docs/architecture/contracts.md` is short and navigational.
- No information loss: content is moved and linkified, not deleted.

## Suggested commit message
`docs: modularize interface contracts`

