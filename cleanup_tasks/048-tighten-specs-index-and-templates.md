# Cleanup Task 048: Tighten specs index and templates

## Goal

Make specs easier to navigate and safer to author by tightening:
- the `docs/specs/README.md` index taxonomy, and
- spec templates so they encode the repo's governance rules (risk, public surface, diagrams).

## Why

Specs are already structured as JTBD and surfaces, but there are avoidable sources of confusion and drift:
- Governance rules (when to use mini vs full spec, mini spec hard limits, public surface restrictions) live in agent standards, not in the specs entrypoint.
- The mini template does not remind authors of the hard limits, so "mini specs" can silently grow into full specs without switching templates.
- Repo-specific doc constraints (no em dashes; Mermaid label text must not include parentheses) are not visible to spec authors at the point of writing.

## Assessment summary (from review task 028)

### What is working

- `docs/specs/README.md` is short and groups specs by major surfaces.
- Full template `_template.md` has the right safety prompts (risk, public surface, security, rollback).
- Specs mostly include `Status`, `Owner`, and `Last updated`, which helps governance and review.

### Key issues to address

- **Governance discoverability:** a spec author reading `docs/specs/README.md` does not see the mini spec limits or when a full spec is required.
- **Template mismatch risk:** a few specs use "mini spec shape" but exceed mini expectations, which increases drift risk.
- **Repo doc constraints are not surfaced:** spec authors can accidentally violate Mermaid label rules and doc typography rules.

## Plan

- Update `docs/specs/README.md`:
  - Add a short "Governance" section that:
    - points to `docs/agent/AGENTS.shared.md` as the canonical rules for mini vs full specs,
    - summarizes the minimum author workflow (declare Risk, declare Public surface, link to invariants/contracts).
  - Keep the index grouped by how readers search (surfaces) and ensure it stays link-first.
- Update `docs/specs/_mini_template.md`:
  - Add a short reminder block (meant to be deleted by the author) for:
    - mini spec hard limits (words and bullets),
    - no diagrams,
    - public surface restriction.
- Update `docs/specs/_template.md`:
  - Add a brief reminder block (meant to be deleted by the author) for:
    - no em dashes in docs,
    - Mermaid label text must not include parentheses.
  - Encourage link-first behavior (specs should not restate contracts owned by `docs/architecture/*`).

## Files to touch

- `docs/specs/README.md`
- `docs/specs/_mini_template.md`
- `docs/specs/_template.md`

## Acceptance criteria

- A spec author can learn the template choice and size limits from `docs/specs/README.md` without hunting.
- Templates make it hard to accidentally violate repo doc constraints.
- The specs index remains short and surface-oriented (no content rewrite of specs in this task).

## Suggested commit message

`docs: tighten specs index and templates`

