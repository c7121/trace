# Cleanup Task 001: Slim the docs portal

## Goal
Make `docs/readme.md` a short "start here" portal that is mostly links, not narrative.

## Why
`docs/readme.md` currently mixes product overview, concepts, glossary, architecture summary, and navigation. This duplicates `docs/architecture/README.md` and encourages restating content instead of linking to canonical owners.

## Scope
In scope:
- Rewrite `docs/readme.md` to be a concise portal.
- Keep links stable and correct.

Out of scope:
- Moving or renaming files.
- Editing the core architecture/spec docs beyond link fixes.

## Plan
- Replace the current sections ("What is Trace", "Concepts", "Architecture") with:
  - 1 to 2 sentence description of Trace.
  - "Pick a path" navigation for common audiences:
    - Implementers: `docs/architecture/README.md`
    - Feature work: `docs/specs/` and `docs/adr/`
    - Deploy and ops: `docs/deploy/` (and `harness/README.md` for Trace Lite)
    - Planning: `docs/plan/README.md`
    - Runbooks: `docs/runbooks/`
- Delete the Job Types table and Glossary from `docs/readme.md` and replace with a link to the owning doc (or remove entirely, to be handled by Task 007).
- Keep the "Where to Look" section only if it adds value beyond the architecture index. Prefer linking to `docs/architecture/README.md` instead of re-listing canonical docs twice.

## Files to touch
- `docs/readme.md`
- Optional: `README.md` (only if it needs link text tweaks)

## Acceptance criteria
- `docs/readme.md` is under ~60 lines and is primarily links.
- `docs/readme.md` does not restate concepts that are already owned by `docs/architecture/*` or `docs/specs/*`.
- No broken local links (including case-sensitive paths).

## Reduction
- Reduce repeated explanations in favor of links to canonical owners.

## Suggested commit message
`docs: slim docs portal`

