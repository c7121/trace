# Cleanup Task 057: Tighten ADR linkability and decision focus

## Goal

Make ADRs easier to use as durable decisions by:
- ensuring each ADR clearly links to the specs and architecture docs it impacts, and
- keeping ADRs decision-focused rather than the only home of normative contracts.

## Why

The ADR set is already small and valuable, but there are a few drift risks:

- Most ADRs do not link to the specs and architecture docs that implement or depend on the decision, so readers and agents have to search.
- A few contract-like details appear only in ADRs (for example constraints that affect DAG config and dataset publishing), which conflicts with the "specs and architecture are the source of truth" hierarchy.

## Plan

- Update `docs/adr/README.md`:
  - Add a short 1-line description per ADR (so the index is scannable without opening each file).
  - Keep the list flat (no new taxonomy unless it helps).
- Add a small `Related` section to each ADR that links to the most relevant canonical docs:
  - Orchestration and lifecycle docs under `docs/architecture/`
  - Contract owners under `docs/architecture/contracts/`
  - Feature surfaces under `docs/specs/`
- Add an explicit "Normative surface" pointer when an ADR mentions a contract-level rule:
  - For example: "The DAG YAML contract is owned by `docs/specs/dag_configuration.md`."
  - Do not duplicate the full contract text in ADRs when a spec already exists.

Note: moving contract-only details out of ADRs and into the owning spec is handled by the relevant spec cleanup tasks (for example `cleanup_tasks/049-tighten-dag-configuration-spec.md`).

## Files to touch

- `docs/adr/README.md`
- `docs/adr/0001-orchestrator.md`
- `docs/adr/0002-networking.md`
- `docs/adr/0003-udf-bundles.md`
- `docs/adr/0004-alert-event-sinks.md`
- `docs/adr/0005-query-results.md`
- `docs/adr/0006-buffered-postgres-datasets.md`
- `docs/adr/0007-input-edge-filters.md`
- `docs/adr/0008-dataset-registry-and-publishing.md`
- `docs/adr/0009-atomic-cutover-and-query-pinning.md`

## Acceptance criteria

- Each ADR has a `Related` section that points to the key owning docs in specs and architecture.
- The ADR index includes 1-line summaries per ADR.
- ADRs remain short and decision-focused; detailed contracts live in the owning spec/architecture docs.

## Suggested commit message

`docs: tighten adr links and index`
