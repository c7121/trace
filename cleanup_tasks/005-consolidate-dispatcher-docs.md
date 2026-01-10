# Cleanup Task 005: Consolidate Dispatcher docs

## Goal
Reduce duplication across Dispatcher documentation by assigning clear owners and trimming repeats.

## Why
Dispatcher behavior and contracts currently appear across:
- Container doc: `docs/architecture/containers/dispatcher.md`
- Lifecycle: `docs/architecture/task_lifecycle.md`
- Contracts: `docs/architecture/contracts.md`
- Specs that are about related surfaces (DAG config, runtime invoker, sinks)

Without strict ownership, the same invariants get rephrased in multiple places.

## Recommendation: pick owners
- `docs/architecture/task_lifecycle.md` owns: leasing, retries, outbox, attempt fencing.
- `docs/architecture/contracts.md` owns: wire contracts, token/claims, endpoint classes.
- `docs/architecture/containers/dispatcher.md` owns: dispatcher responsibilities, internal components, and links.
- Specs own: feature surfaces that depend on Dispatcher (DAG deployment/config, runtime invocation, sinks).

## Plan
- Edit `docs/architecture/containers/dispatcher.md` to be link-first:
  - Keep responsibilities, internal boundaries, and a brief interaction overview.
  - Delete repeated lifecycle semantics (lease, fencing, outbox) and link to `docs/architecture/task_lifecycle.md`.
  - Delete repeated contract sections and link to `docs/architecture/contracts.md`.
- Ensure specs that reference Dispatcher link to the canonical lifecycle and contract docs instead of duplicating text.

## Files to touch
- `docs/architecture/containers/dispatcher.md`
- Optional: small link edits in `docs/specs/*` that currently restate Dispatcher invariants

## Acceptance criteria
- Net word count in `docs/architecture/containers/dispatcher.md` decreases.
- There is a clear link trail from dispatcher doc to lifecycle and contracts.
- No broken links.

## Reduction
- Remove duplicated lifecycle and contract content from the container doc.

## Suggested commit message
`docs: consolidate dispatcher docs`

