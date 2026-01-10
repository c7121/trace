# Cleanup Task 020: Clarify data versioning doc ownership

## Goal
Make it obvious where to look for:
- the behavior contract for incremental processing and reorg handling, and
- the schema that supports it.

## Why
There are two related docs today:
- `docs/architecture/data_versioning.md` (behavior: versions, cursors, invalidations, commit protocol)
- `docs/architecture/data_model/data_versioning.md` (schema: tables that track the above)

Even though this split is intentional, the naming and placement can make `docs/architecture/data_versioning.md` feel like a random top-level file.

## Plan
- Add explicit "Doc ownership" sections to both files:
  - `docs/architecture/data_versioning.md` owns behavior and invariants.
  - `docs/architecture/data_model/data_versioning.md` owns DDL and table-level notes only.
- Add bidirectional links between the two docs and from `docs/architecture/README.md`.
- Optional rename (only if it improves clarity and does not create churn):
  - Rename `docs/architecture/data_versioning.md` to `docs/architecture/incremental_processing.md` (or similar), keep a short stub or update all internal links.
- Ensure no duplication: keep schemas in `data_model/` and keep behavior contracts in the architecture root.

## Files to touch
- `docs/architecture/data_versioning.md`
- `docs/architecture/data_model/data_versioning.md`
- `docs/architecture/README.md`
- Optional: `docs/README.md` if it links directly

## Acceptance criteria
- A reader can answer "where is the behavior contract" vs "where is the schema" quickly.
- The docs do not contradict each other.
- No information loss: content is moved or linked, not deleted.

## Suggested commit message
`docs: clarify data versioning doc ownership`

