# Cleanup Task 064: Tighten agent docs entrypoint

## Goal

Make agent-facing docs under `docs/agent/` discoverable and non-orphaned without adding noise to the main docs portal.

## Why

The `docs/agent/` files are intentional (they are synced by `.github/workflows/sync-agent-standards.yml`), but today they are not linked from any docs index, and they are only referenced by raw paths (not clickable links).

This makes them feel "floating" even though they are important for agents and maintainers.

Orphan audit summary:

| Doc | Reachable from | Recommended action | Canonical owner |
|-----|---------------|-------------------|-----------------|
| `docs/agent/AGENTS.shared.md` | Referenced by `AGENTS.md` as a plain path (not a link) | Link from `AGENTS.md` | `docs/agent/` (agent standards) |
| `docs/agent/checklist.md` | Not linked from any index | Link from `AGENTS.md` | `docs/agent/` (agent workflow) |
| `docs/agent/references.md` | Not linked from any index | Link from `AGENTS.md` | `docs/agent/` (anchor list) |
| `docs/agent/SETUP_SYNC.md` | Not linked from any index | Link from `AGENTS.md` | `docs/agent/` + `.github/workflows/` |
| `docs/architecture/operators/README.md` | Not linked from any index | Leave as-is or delete (no inbound links). See `cleanup_tasks/041-tighten-docs-navigation-entrypoints.md` | `docs/specs/operators/README.md` |

## Plan

- Update `AGENTS.md` (repo root):
  - Replace bare file paths with Markdown links to:
    - `docs/agent/AGENTS.shared.md`
    - `docs/agent/checklist.md`
    - `docs/agent/references.md`
    - `docs/agent/SETUP_SYNC.md`
  - Link to the sync workflow: `.github/workflows/sync-agent-standards.yml`
  - Keep this in `AGENTS.md` (not `docs/README.md`) so the docs portal stays focused on architecture and behavior.

## Files to touch

- `AGENTS.md`

## Acceptance criteria

- A new maintainer or agent can reach all agent docs from `AGENTS.md` via clickable links.
- The main docs portal (`docs/README.md`) remains focused and does not grow a new agent section.

## Suggested commit message

`docs(agent): link agent docs entrypoint`
