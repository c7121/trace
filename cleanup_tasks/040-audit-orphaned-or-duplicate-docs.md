# Review Task 040: Audit orphaned or duplicated docs

## Scope

- Entire `docs/` tree, focused on files not linked from:
  - `docs/README.md`
  - `docs/architecture/README.md`
  - `docs/specs/README.md`
  - `docs/adr/README.md`
  - `docs/deploy/README.md`
  - `docs/examples/README.md`
  - `docs/plan/README.md`
- `docs/agent/` (agent-facing docs)
- `docs/architecture/operators/README.md` (potentially stale)

## Goal

Find any documentation that is effectively "floating" and therefore increases scatter and drift.

## Assessment checklist

- Reachability: can a reader discover this doc from an index page?
- Ownership: does it have a clear home (contracts, JTBD/specs, C4/containers, examples, planning)?
- Duplication: is it repeating content that exists elsewhere?
- Action: for each candidate, pick one:
  - link it from an index, or
  - move it to a better home, or
  - replace it with a short stub that points to the canonical doc, or
  - remove it only if it is redundant and content exists elsewhere.

## Output

- A table of "doc -> reachable from -> recommended action -> canonical owner".
- A list of proposed moves/renames, with a plan to avoid breaking internal links.

