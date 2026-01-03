# AGENTS.md

This repository uses shared agent standards.

Before doing anything else, the agent MUST read and follow:
- docs/agent/AGENTS.shared.md

## Repo overrides (keep short; fill per repo)
- Canonical commands (install/test/lint/build/run):
  - Docs: (none; markdown only)
  - Harness: `cd harness && cargo test` (deps: `docker compose up -d`, then `cargo run -- migrate`)

- Repo-specific constraints (optional):
  - …

Notes:
- Shared standards and templates are in docs/* and SHOULD be treated as normative unless explicitly overridden here.
