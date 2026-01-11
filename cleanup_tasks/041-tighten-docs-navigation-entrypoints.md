# Cleanup Task 041: Tighten docs navigation entrypoints

## Goal

Make high-level docs navigation non-scattered by removing directory links from entrypoints and ensuring each major area has a clear index page.

## Why

Directory links encourage wandering and create scatter. Entry points should route a reader to a single canonical next page.

## Plan

- Replace directory links in entrypoints with links to canonical index pages:
  - `docs/README.md`: use `deploy/README.md`, `specs/README.md`, `architecture/data_model/README.md`, and `architecture/containers/README.md`
  - `docs/architecture/README.md`: link to `data_model/README.md` and `containers/README.md` instead of directories
- Add `docs/architecture/containers/README.md` as a link-first index for container docs.
- Resolve `docs/architecture/operators/README.md`:
  - Prefer a short stub that points to `docs/specs/operators/README.md`, or remove it only if it is unused and redundant.
- Update any inbound links that point to directories or to the orphan operators doc.

## Files to touch

- `docs/README.md`
- `docs/architecture/README.md`
- `docs/architecture/containers/README.md` (new)
- `docs/architecture/operators/README.md` (stub or delete)
- Any docs that link to the above directories or old paths

## Acceptance criteria

- Entry points do not link to directories where an index page exists.
- `docs/architecture/containers/README.md` exists and links to each container doc.
- No important navigation points to `docs/architecture/operators/README.md` unless it is the intended stub.

## Suggested commit message

`docs: tighten navigation entrypoints`

