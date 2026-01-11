# Cleanup Task 054: Tighten metadata and lineage spec

## Goal

Make `docs/specs/metadata.md` a link-first cross-cutting spec that helps readers find the canonical owners for:
- dataset registry and publishing,
- dataset version pinning and rollback,
- lineage and orchestration state,
- schema mapping for metadata tables.

## Why

`docs/specs/metadata.md` is useful, but it currently increases drift risk:

- It repeats cross-cutting constraints (at-least-once execution, Postgres state vs data boundary) that already have canonical owners under `docs/architecture/`.
- It claims "Canonical DDL lives in" a set of `docs/architecture/data_model/*` docs, but the repo declares `harness/migrations/*` as the schema source of truth. This can mislead implementers.
- The public surface description is vague and does not link to the concrete user-facing dataset endpoints or the task completion contract where most metadata is written.

## Plan

- Make the spec explicitly link-first:
  - Replace repeated constraints with short statements that link to the canonical owners:
    - `docs/architecture/task_lifecycle.md`
    - `docs/architecture/db_boundaries.md`
    - `docs/architecture/data_versioning.md`
    - `docs/architecture/data_model/README.md`
  - Add a small "Doc map" section that points to:
    - ADR 0008 (registry and publishing)
    - ADR 0009 (cutover and query pinning)
    - `docs/architecture/user_api_contracts.md` (user-facing dataset discovery routes)
    - `docs/architecture/contracts/task_scoped_endpoints.md` (task completion metadata write path)
- Fix the schema ownership statement:
  - Replace "Canonical DDL lives in" with "Schema mapping docs live in" and make `harness/migrations/state/` and `harness/migrations/data/` the declared canonical sources.
- Keep the unique value of this doc:
  - Preserve the "what the system tracks" list, but tighten phrasing so it is a navigational index rather than a second copy of architecture docs.

## Files to touch

- `docs/specs/metadata.md`

## Acceptance criteria

- The spec has no incorrect "canonical DDL" claim and points readers to migrations as the source of truth.
- Cross-cutting constraints are link-first (no repeated mini-versions of architecture docs).
- A reader can find the dataset API routes and the task completion metadata contract in 1-2 clicks from this doc.

## Suggested commit message

`docs: tighten metadata spec`
