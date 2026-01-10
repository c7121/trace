# Review Task 024: Security model and contracts

## Scope

- `docs/architecture/security.md`
- `docs/architecture/contracts.md`
- `docs/architecture/contracts/`
- `docs/architecture/user_api_contracts.md`

## Goal

Critically assess whether trust boundaries, authn/authz, and wire-level contracts are easy to find and unambiguous.

## Assessment checklist

- Ownership: which doc is the source of truth for each public surface?
- Boundary: does the security model clearly separate trusted vs untrusted components?
- Duplication: are the same endpoints or payload schemas defined in multiple places?
- Consistency: does task fencing and attempt semantics match lifecycle docs and code?
- Structure: do contract docs read like references (inputs/outputs/invariants), not tutorials?

## Output

- A table of "surface -> owner doc -> dependent docs".
- A list of any conflicting statements across the scope.
- Recommendations to reduce duplication (link-first, single canonical schema per surface).

