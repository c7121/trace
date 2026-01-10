# Cleanup Task 015: De-drift data model docs

## Goal
Remove contradictions and drift risk in `docs/architecture/data_model/` by clarifying what is canonical and simplifying where needed.

## Why
The data model docs currently claim "Canonical DDL" and include multiple overlapping representations (DDL sketches and ERDs). Some of these disagree with each other (for example task attempt fields and lease fields).

Under a design-by-contract lens, schema docs must either:
- be canonical and stay in sync with migrations, or
- be explicitly non-canonical, link to the real source of truth, and avoid details that easily drift.

## Plan
- Define the source of truth for schema as `harness/migrations/` and update data model docs to reflect that.
- Reduce drift by making ERD docs relationship-focused:
  - Keep entity relationships in Mermaid ER diagrams.
  - Remove (or heavily reduce) per-entity column listings that duplicate DDL and drift quickly.
- Update headings and wording that currently claim "Canonical DDL" when the doc is not guaranteed to match migrations.
- Ensure there are no internal contradictions between:
  - `docs/architecture/data_model/orchestration.md`
  - `docs/architecture/data_model/erd_state.md`
  - `docs/architecture/data_model/erd_data.md`

## Files to touch
- `docs/architecture/data_model/orchestration.md`
- `docs/architecture/data_model/erd_state.md`
- `docs/architecture/data_model/erd_data.md`
- Optional: add a short `docs/architecture/data_model/README.md` that states scope and source of truth.

## Acceptance criteria
- Data model docs do not claim to be canonical unless they are kept in sync with migrations.
- No doc-to-doc contradictions remain within `docs/architecture/data_model/`.
- Readers have a clear pointer to the canonical schema source (`harness/migrations/`).

## Suggested commit message
`docs: de-drift data model docs`

