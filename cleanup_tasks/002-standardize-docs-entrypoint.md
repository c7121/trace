# Cleanup Task 002: Standardize the docs entrypoint

## Goal
Choose one canonical docs entrypoint and standardize naming to reduce confusion and duplicated "start here" pages.

## Recommendation
Make `docs/README.md` the canonical docs entrypoint.

## Plan
- Rename `docs/readme.md` to `docs/README.md`.
- Update repo links to point to `docs/README.md`:
  - `README.md`
  - any references inside `docs/`
- Decide what to do with the old path:
  - Preferred: keep a small `docs/readme.md` stub that links to `docs/README.md` (so old links still work).
  - Alternative: delete `docs/readme.md` and update all links.

## Files to touch
- `docs/readme.md` (rename or stub)
- `docs/README.md` (new canonical entrypoint)
- `README.md` and any docs that reference the old path

## Acceptance criteria
- `README.md` points at `docs/README.md`.
- There is exactly one canonical docs entrypoint.
- Local link check passes, including case-sensitive link paths.

## Reduction
- Reduce navigation ambiguity and duplicated "start here" pages.

## Suggested commit message
`docs: standardize docs entrypoint`

