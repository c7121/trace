# Cleanup Task 017: Fix Mermaid labels with parentheses

## Goal
Ensure Mermaid diagram labels do not include parentheses in label text.

## Why
Repo docs rules require that Mermaid label text does not contain parentheses. Parentheses are allowed only for Mermaid shape syntax.

If label text contains parentheses, diagrams become inconsistent with the repo standard and can be harder to read.

## Plan
- Scan all Mermaid fenced blocks under `docs/` for node labels or edge labels that include `(` or `)` in the label text.
- Rewrite label text to remove parentheses while preserving meaning.
  - Example: `Postgres state (control plane)` becomes `Postgres state - control plane`.
- Do not modify parentheses used purely for Mermaid shapes.

## Files to touch
- Any Markdown files under `docs/` that contain Mermaid diagrams with parentheses inside label text.

## Acceptance criteria
- No Mermaid label text contains parentheses.
- Mermaid shape syntax remains valid.

## Suggested commit message
`docs: fix mermaid label parentheses`

