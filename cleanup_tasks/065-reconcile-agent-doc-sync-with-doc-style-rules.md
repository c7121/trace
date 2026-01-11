# Cleanup Task 065: Reconcile agent doc sync with doc style rules

## Goal

Keep the "no em dashes in docs" rule enforceable even though `docs/agent/*` is synced from an upstream repository.

## Why

`docs/agent/AGENTS.shared.md` currently contains em dashes. Even if we fix them locally, the weekly sync workflow can reintroduce them.

This creates a permanent mismatch between the repo's doc style rules and the synced agent standards.

## Plan

Pick one of these options and make it explicit:

Option A (recommended): normalize after sync
- Update `.github/workflows/sync-agent-standards.yml` to replace em dashes with hyphens in `docs/agent/*.md` after syncing, before creating the PR.
- Add a follow-up check step that fails the workflow if any em dashes remain under `docs/agent/`.

Option B: exempt synced agent docs
- Update the repo doc style rule in `AGENTS.md` to explicitly exempt `docs/agent/` because it is synced content.

## Files to touch

- `.github/workflows/sync-agent-standards.yml` (Option A)
- `AGENTS.md` (Option B or to document the decision)

## Acceptance criteria

- The repo doc style rule and the synced agent docs stop conflicting.
- The decision is documented in `AGENTS.md` so future contributors do not re-litigate it.

## Suggested commit message

`chore(agent): reconcile doc style with sync`

