# Cleanup Task 061: Shrink milestones ledger without losing history

## Goal

Reduce section count and drift risk in `docs/plan/milestones.md` while preserving information by:
- keeping `docs/plan/milestones.md` as the short canonical ledger and forward-looking plan, and
- moving detailed completed milestone notes into a separate archive file.

## Why

`docs/plan/milestones.md` currently mixes:
- a ledger index (completed milestone table + planned milestone table), and
- detailed milestone writeups for completed milestones.

Those detailed writeups can be valuable history, but they make the file long and easy to misread as current behavior (even though specs and architecture docs are the source of truth for behavior).

## Plan

- Create `docs/plan/milestones_archive.md`:
  - Move the detailed completed milestone sections (for example Milestones 8-17) into this archive file.
  - Keep their content intact (no information loss), and keep any Context links and STOP gates.
- Update `docs/plan/milestones.md`:
  - Keep:
    - Completed milestones table (tags and summaries)
    - Planned milestones and Security Gate S1 sections
  - Add a short pointer:
    - "Detailed completed milestone notes live in `docs/plan/milestones_archive.md`."
  - Add a short disclaimer:
    - "Milestone notes are historical; current behavior is defined by `docs/architecture/*` and `docs/specs/*`."

## Files to touch

- `docs/plan/milestones.md`
- `docs/plan/milestones_archive.md` (new)

## Acceptance criteria

- `docs/plan/milestones.md` remains the canonical ledger but is materially shorter and easier to scan.
- No milestone history is lost; detailed notes are preserved in the archive file.
- A reader is directed to specs and architecture docs for current behavior, avoiding drift.

## Suggested commit message

`docs: shrink milestones ledger`
