# Cleanup Task 004: Consolidate Query Service docs

## Goal
Reduce duplication across Query Service documentation by assigning clear owners and trimming repeats.

## Why
Query Service material currently spans:
- Container description: `docs/architecture/containers/query_service.md`
- Data model: `docs/architecture/data_model/query_service.md`
- Specs: `docs/specs/query_service_user_query.md`, `docs/specs/query_service_task_query.md`
- Contracts: `docs/architecture/contracts.md`

This is useful coverage, but it is easy for the same story to be re-explained four times.

## Recommendation: pick owners
- Container doc owns: responsibilities, dependencies, trust boundaries, and links.
- Specs own: user-facing and task-facing API surfaces and semantics.
- Data model doc owns: tables and schema constraints only.
- Contracts doc owns: shared wire contracts and token/claims rules used by multiple services.

## Plan
- Edit `docs/architecture/containers/query_service.md` to be link-first:
  - Keep responsibilities, key invariants, and trust boundary summary.
  - Delete repeated endpoint-by-endpoint semantics and point to the relevant spec and contract sections instead.
  - Add a short "See also" section with the exact docs above.
- Edit the two Query Service spec files to remove any content that is purely container responsibilities or repeated trust boundary narrative (and link to the container and security docs instead).
- Edit `docs/architecture/data_model/query_service.md` to be schema-only (no repeated behavior).

## Files to touch
- `docs/architecture/containers/query_service.md`
- `docs/specs/query_service_user_query.md`
- `docs/specs/query_service_task_query.md`
- `docs/architecture/data_model/query_service.md`
- Optional: link cleanup in `docs/architecture/contracts.md` if it contains Query Service narrative that belongs elsewhere

## Acceptance criteria
- Each doc sticks to its owner scope (container vs spec vs data model vs contracts).
- Net word count across these docs decreases.
- No broken links.

## Reduction
- Remove repeated explanations and keep a single owner per concern.

## Suggested commit message
`docs: consolidate query service docs`

