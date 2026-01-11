# Cleanup Task 060: Tighten planning docs entrypoints and boundaries

## Goal

Make the planning docs under `docs/plan/` easier to navigate and lower drift by:
- making `docs/plan/README.md` a real index (including `trace_lite.md`), and
- stating clear rules for what belongs in plan docs vs specs and architecture docs.

## Why

Planning docs are useful, but they can become a second, conflicting source of truth if they restate behavior that belongs in `docs/specs/` or `docs/architecture/`.

Current issues:
- `docs/plan/README.md` does not link to `docs/plan/trace_lite.md`, even though it is referenced by deploy docs and milestones.
- Plan docs do not explicitly state that they are sequencing-only and must be link-first to the canonical owners for behavior and contracts.

## Plan

- Update `docs/plan/README.md`:
  - Add `Trace Lite runner notes: trace_lite.md` to the index.
  - Add a short "Rules" section:
    - Plan docs are sequencing and review gates only.
    - Plan docs must not define canonical behavior contracts.
    - When plan docs mention an invariant or surface, they must link to the owning spec or architecture doc.
- Update `docs/plan/backlog.md` (small):
  - Add a short preface: backlog items should point to an owning doc or be promoted into a spec or ADR when they become actionable.
- Update `docs/plan/trace_lite.md` (small):
  - Keep it as "what trace-lite does / does not do".
  - Ensure it stays link-first to:
    - `docs/examples/lite_local_cryo_sync.md` (end-to-end runbook)
    - `harness/README.md` (harness commands and verification)
    - `docs/architecture/security.md` or `docs/adr/0002-networking.md` for egress constraints
  - Avoid duplicating long runbook steps already owned by the example doc.

## Files to touch

- `docs/plan/README.md`
- `docs/plan/backlog.md`
- `docs/plan/trace_lite.md`

## Acceptance criteria

- `docs/plan/README.md` routes a reader to all planning docs in 1 click.
- Plan docs are explicitly sequencing-only and link-first to canonical behavior owners.
- `docs/plan/trace_lite.md` remains useful but does not duplicate the full end-to-end runbook.

## Suggested commit message

`docs: tighten planning docs entrypoints`
