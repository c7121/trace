# Review Task 027: Operations vs deployment docs boundary

## Scope

- `docs/architecture/operations.md`
- `docs/architecture/dag_deployment.md`
- `docs/deploy/`

## Goal

Critically assess whether operations, deployment, and configuration guidance is coherent, with minimal duplication and clear doc ownership.

## Assessment checklist

- Ownership: what is architecture-level ops guidance vs deploy-time steps vs examples?
- Duplication: are deployment profiles and operational guidance repeated in multiple places?
- Actionability: do deploy docs contain concrete, minimal steps and links to prerequisites?
- Drift risk: do docs reference "current implementation" without a stable anchor?
- Scope: do deploy docs mix in product/spec content that belongs elsewhere?

## Output

- A critique of the current split and where duplication remains.
- Proposed ownership statements and cross-links to make the split obvious.
- A list of sections that should be moved into examples or removed as redundant.

