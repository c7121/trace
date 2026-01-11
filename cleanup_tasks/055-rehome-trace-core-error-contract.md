# Cleanup Task 055: Rehome trace-core error contract decision

## Goal

Reduce docs scatter by moving the trace-core error contract decision to the correct long-lived home and making its status unambiguous.

## Why

`docs/specs/trace_core_error_contract.md` describes an internal Rust API design decision, not a JTBD feature surface.

It also appears to already be implemented: `crates/trace-core/src/lib.rs` defines `trace_core::{Error, Result<T>}` and there are no `anyhow::Result` exports in the crate.

Keeping this as a "spec" under `docs/specs/` makes it harder for readers and agents to know where to look for durable decisions.

## Plan

- Convert the decision into an ADR:
  - Add `docs/adr/0010-trace-core-error-contract.md` (use the ADR template style).
  - Include the original rationale and the "no anyhow in public API" rule as the durable decision.
  - Mark it as Accepted and note it is implemented.
- Update indexes:
  - Add the new ADR to `docs/adr/README.md`.
  - Update `docs/specs/README.md` to remove the "Error contract" entry (or replace it with a link to the ADR if we want a cross-cutting pointer).
- Remove the old spec doc without information loss:
  - Delete `docs/specs/trace_core_error_contract.md` after the ADR exists.

## Files to touch

- `docs/adr/README.md`
- `docs/adr/0010-trace-core-error-contract.md` (new)
- `docs/specs/README.md`
- `docs/specs/trace_core_error_contract.md` (delete)

## Acceptance criteria

- The decision is recorded as an ADR and is discoverable from the ADR index.
- The specs index no longer suggests this is a feature spec.
- There is no information loss: the content is preserved in the ADR, and the old file is removed only after the ADR exists.

## Suggested commit message

`docs: rehome trace-core error contract decision`
