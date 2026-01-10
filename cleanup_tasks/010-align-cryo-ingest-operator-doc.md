# Cleanup Task 010: Align cryo_ingest operator doc

## Goal
Make the `cryo_ingest` operator documentation accurate for the current implementation and contracts.

## Why
The current operator doc is inconsistent with the code and specs:
- Range semantics and payload fields have drifted across docs and the implementation.
- It describes a manual job config shape that does not match how `chain_sync` plans `cryo_ingest` tasks today.

## Plan
- Update `docs/architecture/operators/cryo_ingest.md` to:
  - Declare the canonical range semantics as implemented today and align all examples.
  - Describe the current task payload fields used in Lite:
    - `dataset_uuid`, `chain_id`, `range_start`, `range_end`, `config_hash`
    - optional: `dataset_key`, `cryo_dataset_name`, `rpc_pool`
  - Correct the output prefix description to match the worker publication scheme.
  - Reduce section sprawl by removing or replacing outdated "Execution" and "Example DAG Config" content with links to:
    - `docs/specs/chain_sync_entrypoint.md`
    - `docs/examples/chain_sync.monad_mainnet.yaml`

## Files to touch
- `docs/architecture/operators/cryo_ingest.md`

## Acceptance criteria
- No remaining claims that range end is inclusive.
- Operator doc matches the current payload and publication behavior.
- The doc is shorter and link-first (no duplicated planning narrative).

## Suggested commit message
`docs: align cryo_ingest operator doc`
