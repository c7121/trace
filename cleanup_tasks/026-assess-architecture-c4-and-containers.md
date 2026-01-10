# Review Task 026: C4 and container docs

## Scope

- `docs/architecture/c4.md`
- `docs/architecture/containers/`

## Goal

Critically assess whether the system tour is cohesive: C4 tells the story, container docs deepen it without repeating or drifting.

## Assessment checklist

- C4 purity: does `c4.md` stay a tour, not a dumping ground for implementation details?
- Container ownership: does each container doc have clear responsibilities, dependencies, and non-responsibilities?
- Duplication: are the same responsibilities described in multiple container docs or repeated from specs?
- Link-first: do container docs link out to contracts/specs/runbooks instead of re-explaining them?
- Missing pieces: are any deployed units missing a container doc or incorrectly represented?

## Output

- A duplication list between `c4.md` and container docs.
- A set of structural changes to make C4 the top narrative and containers the references.
- A set of recommended "Related" links to standardize at the end of container docs.

