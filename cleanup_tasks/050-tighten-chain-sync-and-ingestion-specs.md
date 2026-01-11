# Cleanup Task 050: Tighten chain sync and ingestion specs

## Goal

Make the ingestion story cohesive and non-duplicative by clarifying doc ownership across:
- `docs/specs/chain_sync_entrypoint.md` (chain sync job surface and planning invariants)
- `docs/specs/ingestion.md` (ingestion patterns and how to choose operators)
- `docs/specs/cryo_library_adapter.md` (implementation spike, not a contract)

## Why

These docs are individually useful, but they currently have drift and duplication risks:
- `docs/specs/chain_sync_entrypoint.md` restates several platform-wide invariants (task lifecycle, Query Service SQL gating) instead of linking to canonical owners.
- `docs/specs/ingestion.md` describes bootstrap scheduling as an external range manifest plus `range_splitter`, which can be read as the recommended v1 orchestration path even though `chain_sync` is intended to internalize planning.
- `docs/specs/cryo_library_adapter.md` is a design spike but is indexed like a first-class ingestion surface, which can confuse readers about what is normative.

## Assessment summary (from review task 030)

### Ownership statements (recommended)

- `docs/specs/chain_sync_entrypoint.md` owns:
  - the admin surface for defining a `chain_sync` job (file shape, fields),
  - planner and state invariants (idempotency, monotonic cursors, range ledger),
  - the required task payload contract for `cryo_ingest` tasks planned by `chain_sync`.
- `docs/specs/ingestion.md` owns:
  - ingestion modes (tip follower vs bounded bootstrap),
  - how ingestion fits the versioning and invalidation model,
  - links to the concrete operator specs.
- `docs/specs/cryo_library_adapter.md` owns:
  - a proposed implementation boundary for embedding Cryo as a library and streaming artifacts,
  - explicit non-goals and the fact that it is not a v1 behavior contract.

### Duplication map

- Task lifecycle and at-least-once semantics:
  - currently repeated in `docs/specs/chain_sync_entrypoint.md`
  - canonical owners: `docs/architecture/invariants.md` and `docs/architecture/task_lifecycle.md`
- Query Service SQL gating and fail-closed story:
  - repeated in `docs/specs/chain_sync_entrypoint.md`
  - canonical owners: `docs/specs/query_sql_gating.md` and `docs/specs/query_service_task_query.md` plus `docs/architecture/containers/query_service.md`
- Range semantics ([start, end) end-exclusive):
  - referenced in multiple places
  - canonical owners: `docs/architecture/contracts/task_scoped_endpoints.md` and `docs/architecture/data_versioning.md`

## Plan

- In `docs/specs/chain_sync_entrypoint.md`:
  - Add a short "Doc ownership" section near the top that explicitly links to the canonical owners listed above.
  - Convert raw file path references and ADR references (for example "ADR-0002") into Markdown links.
  - Keep only the chain-sync-specific invariants in this doc and replace repeated platform-wide explanations with links.
  - Make the chosen config shape explicit:
    - If Option B is the v1 choice, treat Option A as the compiled/internal representation and collapse it to a short note to reduce headings.
  - Add a single link-first subsection for range semantics that points to the canonical definition and avoids re-explaining it.
  - Keep the ms history, but move it to a short "History" subsection or link it into `docs/plan/` so it does not dominate the contract narrative.
- In `docs/specs/ingestion.md`:
  - Add a short "Bootstrap orchestration (v1)" section that points to `docs/specs/chain_sync_entrypoint.md` as the canonical system-managed planner path.
  - Keep the "generic DAG with range_splitter" path as an explicit alternative, but label it as manual or future composition, not the primary v1 story.
- In `docs/specs/cryo_library_adapter.md`:
  - Add a visible "Spike" ownership note up front: this is an implementation exploration, not a behavior contract.
  - Link to the canonical publication and task payload contracts it must preserve (for example `docs/architecture/contracts/task_scoped_endpoints.md` and `docs/specs/operators/cryo_ingest.md`).
  - Optionally: adjust `docs/specs/README.md` to label this doc as a spike (non-normative) so readers do not treat it as required reading for ingestion.

## Files to touch

- `docs/specs/chain_sync_entrypoint.md`
- `docs/specs/ingestion.md`
- `docs/specs/cryo_library_adapter.md`
- `docs/specs/README.md` (optional, for labeling)

## Acceptance criteria

- Each doc has an explicit ownership statement and does not restate platform-wide contracts owned elsewhere.
- Bootstrap ingestion guidance does not contradict the "no external planning loops" `chain_sync` invariant.
- Range semantics are defined in one canonical place and referenced consistently.
- Cryo library adapter is clearly marked as a spike and not confused with a v1 contract.

## Suggested commit message

`docs: tighten chain sync and ingestion specs`

