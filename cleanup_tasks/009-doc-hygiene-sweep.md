# Cleanup Task 009: Docs hygiene sweep

## Goal
Apply a mechanical hygiene pass across docs to reduce friction and keep rendering stable.

## What to do
- Replace em dashes (U+2014) in docs with hyphens or colons.
- Re-check Mermaid diagrams for label punctuation constraints:
  - Avoid parentheses in label text.
  - Keep edge labels short and ASCII.
- Validate local links (including case-sensitive paths).
- Remove dead or low-value external reference links from portals and indexes (keep them in the doc where they are actually used).

## Files to touch
Potentially many Markdown files under `docs/` (and optionally root `README.md`).

## Acceptance criteria
- No em dashes remain in `docs/`.
- Mermaid diagrams render under strict renderers (no parentheses in label text).
- No broken local links.

## Reduction
- Reduce small sources of doc drift and tooling incompatibility.

## Suggested commit message
`docs: apply hygiene sweep`
