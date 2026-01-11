# Cleanup Task 042: Tighten architecture index core concepts

## Goal

Reduce implementer lookup friction by making `docs/architecture/README.md` more complete and more link-precise.

## Why

The architecture index is already a strong entrypoint, but:
- it relies on directory links in a few places, and
- a couple of core nouns are used widely but not defined in the core concept list.

## Plan

- Update the Core concepts list in `docs/architecture/README.md` to include:
  - **Dataset**: identity, versioning, and where dataset pointers live
  - **Operator**: the stable “what code runs” surface for jobs
- Tighten the Canonical documents list:
  - Add `db_boundaries.md` as a recommended early read (DB split is a core boundary).
  - Replace directory links with index pages where they exist (or will exist after Task 041).
- Add a short “best next hop” hint per concept (one link each) to reduce wandering.

## Files to touch

- `docs/architecture/README.md`
- Possibly `docs/README.md` if a link needs to be updated after the change

## Acceptance criteria

- Core concepts include the nouns that recur throughout the docs without forcing readers to hunt.
- The reading order points to files, not directories, where an index exists.
- No information is deleted, only clarified and linked.

## Suggested commit message

`docs: tighten architecture index core concepts`
