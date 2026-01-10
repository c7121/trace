# Review Task 021: Docs portal and entrypoints

## Scope

- `README.md`
- `docs/README.md`
- `docs/architecture/README.md`
- `docs/specs/README.md`
- `docs/adr/README.md`
- `docs/deploy/README.md`
- `docs/examples/README.md`
- `docs/plan/README.md`
- `harness/README.md`

## Goal

Critically assess whether the first-click experience is coherent and non-scattered for each audience:
- implementer
- spec author
- operator author
- deployer/operator
- Trace Lite user

## Assessment checklist

- Ownership: does each entrypoint state what it owns and what it intentionally does not?
- Audience fit: can a reader quickly choose the right path?
- Duplication: where are we repeating the same story in different places?
- Link quality: are links mostly Markdown links (not raw paths) and do they land on a strong next page?
- Naming: do file and section names match the mental model (contracts, JTBD, C4, implementation)?
- Orphan risk: are there important docs not reachable from these entrypoints?

## Output

- A short critique (scattered vs cohesive).
- A proposed navigation map: "start here" per persona, with 1-2 hops.
- A list of recommended edits (move, link, rename, or split) with zero information loss.

