# AGENTS.md

This repository uses shared agent standards.

Before doing anything else, the agent MUST read and follow:
- docs/agent/AGENTS.shared.md

## Repo overrides (keep short; fill per repo)
- Canonical commands (install/test/lint/build/run):
  - Docs: (none; markdown only)
  - Harness: `cd harness && cargo test` (deps: `docker compose up -d`, then `cargo run -- migrate`)

- Repo-specific constraints (optional):
  - No em dashes (—) in docs; use hyphens (-) or colons
  - Mermaid labels: do not include parentheses `()` in label text (node labels or edge labels); parentheses used only for Mermaid shape syntax are OK
  - Handoff artifacts: if you must write files outside the repo, only write to the repo parent directory (`../`); do not write to any path above `../` or to `/tmp`
  - Never read from or source `.env` files; always pass env vars inline on the command line

Notes:
- Shared standards and templates are in docs/* and SHOULD be treated as normative unless explicitly overridden here.

## Milestone workflow

Milestones are tracked in `docs/plan/milestones.md`. Completed milestones are tagged as `ms/<N>` (e.g., `ms/7`).

### Source of truth hierarchy

The source of truth for behavior and invariants is (in order):
1. `docs/architecture/*` (contracts, lifecycle, containers)
2. `docs/specs/*` (feature surfaces)
3. `docs/adr/*` (decisions)

### Context links requirement

Every milestone MUST include a **Context links** list (repo-relative paths) to the docs/specs/contracts that define the milestone's intended behavior. If a milestone adds or edits any docs, update its Context links.

### Harness "green" command

From `harness/`:

```bash
docker compose down -v
docker compose up -d
cargo run -- migrate
cargo test -- --nocapture
```

Keep the harness green. If it breaks, fix it before adding features.

### STOP gates

- Treat each milestone as a review gate.
- Make changes in **small commits** with a clear verification command each.
- At each 🛑 STOP point, share:
  - a zip of the repo including `.git` - use a timestamped filename, e.g. `trace-2026-01-08T1430.zip`
  - output of `cd harness && cargo test -- --nocapture`
  - `git log --oneline -n 30`
  - a short note: "what changed" + "what you want reviewed"
