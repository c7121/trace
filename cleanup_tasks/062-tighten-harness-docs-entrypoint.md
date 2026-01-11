# Cleanup Task 062: Tighten harness docs entrypoint

## Goal

Make the harness docs clearly harness-scoped (contract-freeze integration harness) while still being a good entrypoint for:
- how to run the harness in isolation, and
- where to go for full Trace Lite end-to-end runs.

## Why

Today the harness docs are minimal and useful, but they can accidentally be treated as a Trace Lite "how to run everything" guide. That creates scatter and drift because the canonical end-to-end runbook lives under `docs/examples/`.

There are also a few drift issues:
- `harness/AGENT_TASKS.md` links to a non-existent `docs/plan/plan.md`.
- Some contract references should point at specific contract docs under `docs/architecture/contracts/`.
- Harness docs contain em dashes, which this repo's doc standards disallow.

## Plan

- Update `harness/README.md`:
  - Add a short "What this is / what this is not" section:
    - This is the contract-freeze harness, not the canonical Trace Lite runbook.
    - Link to `docs/examples/lite_local_cryo_sync.md` for end-to-end Trace Lite.
    - Link to `docs/plan/trace_lite.md` for runner semantics.
  - Add a "Green harness command" section that matches `AGENTS.md`.
  - Replace any em dashes with hyphens or colons.
- Update `harness/NOTES.md` to be link-first:
  - Keep harness-specific deltas (HS256 dev secret, anonymous MinIO).
  - Add links to canonical owner docs for:
    - capability tokens and task-scoped endpoint contracts
    - security model and trust boundary notes
    - Cryo invocation and block range semantics
- Update `harness/AGENT_TASKS.md`:
  - Fix the broken plan link (use `docs/plan/README.md` or `docs/plan/milestones.md`).
  - Update contract links to point at the specific contract docs under `docs/architecture/contracts/`.
  - Replace em dashes with hyphens or colons.
  - Ensure verification commands match the repo's harness "green" command (including `-- --nocapture`).

## Files to touch

- `harness/README.md`
- `harness/NOTES.md`
- `harness/AGENT_TASKS.md`

## Acceptance criteria

- Harness docs are explicit about scope and link to the canonical Trace Lite runbook.
- No harness docs reference missing files.
- Harness docs are link-first to the canonical owner docs, reducing drift.
- No em dashes remain in the touched harness docs.

## Suggested commit message

`docs(harness): tighten harness docs entrypoint`

