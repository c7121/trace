# Cleanup Task 063: Fix Trace Lite entrypoint links

## Goal

Make the top-level entrypoints route a reader to the canonical Trace Lite runbook first, and treat the harness docs as a secondary, harness-only reference.

## Why

Right now, a reader starting from repo entrypoints can end up in `harness/README.md` without being directed to the canonical end-to-end Trace Lite runbook in `docs/examples/`. That increases scatter and makes it easier for "how to run" guidance to drift.

## Plan

- Update the repo root `README.md`:
  - Replace the "Trace Lite harness" link with:
    - Trace Lite runbook: `docs/examples/lite_local_cryo_sync.md`
    - Harness: `harness/README.md` (contract-freeze integration harness)
- Update `docs/README.md`:
  - Under "If you are using Trace Lite", link to:
    - `docs/examples/lite_local_cryo_sync.md` (canonical runbook)
    - `docs/plan/trace_lite.md` (runner semantics)
    - `harness/README.md` (harness-only docs and verification)
- Search for any other "start here for Trace Lite" links that point to harness docs and replace with the runbook link (keep harness as a secondary link where relevant).

## Files to touch

- `README.md`
- `docs/README.md`
- Any docs that incorrectly use `harness/README.md` as the primary Trace Lite runbook link

## Acceptance criteria

- A new reader looking for Trace Lite runs lands in `docs/examples/lite_local_cryo_sync.md` from repo entrypoints.
- Harness docs remain reachable, but are clearly positioned as harness-only.

## Suggested commit message

`docs: fix trace lite entrypoint links`

