# Review Task 023: Architecture correctness and lifecycle docs

## Scope

- `docs/architecture/invariants.md`
- `docs/architecture/task_lifecycle.md`
- `docs/architecture/event_flow.md`

## Goal

Critically assess the correctness narrative: failure modes, idempotency, retries, leasing, outbox semantics, and where these truths are defined.

## Assessment checklist

- Ownership: are invariants only in one place, and do other docs defer to them?
- Completeness: are the key failure cases covered (retries, partial writes, dupes, out-of-order)?
- Drift risk: are we restating protocol details in multiple places?
- Boundary clarity: does the lifecycle doc align with the contracts and worker behavior?
- Structure: are sections easy to scan, or too narrative and repetitive?

## Output

- A duplication map: where lifecycle behavior is described more than once.
- Recommendations to make "invariants first" unavoidable without bloating docs.
- A list of specific sections to move or replace with links.

