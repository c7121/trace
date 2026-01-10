# Cleanup Task 007: Consolidate milestone micro-specs

## Goal
Reduce doc count and browsing overhead by removing tiny milestone-specific spec files and folding their content into the milestone ledger.

## Candidates
These are currently very small and milestone-titled:
- `docs/specs/dispatcher_extraction.md`
- `docs/specs/sink_extraction.md`
- `docs/specs/runtime_invoker.md`
- `docs/specs/lite_chain_sync_planner.md`
- `docs/specs/trace_core_error_contract.md` (small, but might be a durable contract and worth keeping)

## Recommendation
Fold truly milestone-specific content into `docs/plan/milestones.md`, and keep only durable, long-lived specs in `docs/specs/`.

## Plan
- For each candidate, decide whether it is:
  - Milestone planning only: move key bullets into the milestone entry in `docs/plan/milestones.md`, then delete the spec file.
  - Durable spec: keep it in `docs/specs/` and remove milestone framing from the title.
- Update any links that referenced the deleted files.

## Files to touch
- `docs/plan/milestones.md`
- a subset of the candidate spec files above
- link updates across `docs/`

## Acceptance criteria
- Fewer tiny milestone spec files exist under `docs/specs/`.
- The milestone ledger remains the single place to understand milestone sequencing and status.
- No broken links.

## Reduction
- Reduce file count and eliminate "spec sprawl" for milestone notes.

## Suggested commit message
`docs: consolidate milestone micro-specs`

