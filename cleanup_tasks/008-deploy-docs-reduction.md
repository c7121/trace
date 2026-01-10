# Cleanup Task 008: Reduce and reorg deploy docs

## Goal
Make deployment documentation easier to navigate by reducing overlap and creating one clear "how to deploy" path.

## Why
Deployment content is spread across:
- `docs/deploy/infrastructure.md`
- `docs/deploy/deployment_profiles.md`
- `docs/deploy/monitoring.md`
- `docs/examples/lite_local_cryo_sync.md`

These likely mix "AWS production deployment", "Trace Lite harness", and "local data sync" concerns.

## Recommendation
- Keep `docs/deploy/` focused on AWS deployment and ops.
- Move local/lite workflows into either `harness/README.md` or `docs/examples/` (depending on whether it is a harness procedure or an operator-facing example).

## Plan
- Add a short `docs/deploy/README.md` as the only entrypoint (or fold into the docs portal if you want fewer files).
- Split concerns:
  - AWS infrastructure and boundaries: `docs/deploy/infrastructure.md`
  - Profiles and knobs: `docs/deploy/deployment_profiles.md`
  - Monitoring signals and dashboards: `docs/deploy/monitoring.md`
  - Local Cryo sync: move to `docs/examples/` or to `harness/` if it is strictly a harness dependency
- Remove duplicate explanations and replace them with links between these docs.

## Files to touch
- `docs/deploy/*`
- Optional: `docs/examples/*` or `harness/README.md` if moving local instructions

## Acceptance criteria
- There is a single entrypoint for deploy docs with a clear reader path.
- Net word count in deploy docs decreases.
- No broken links.

## Reduction
- Reduce repeated deployment narrative and keep one owner per deploy concern.

## Suggested commit message
`docs: reduce deploy docs and clarify entrypoint`
