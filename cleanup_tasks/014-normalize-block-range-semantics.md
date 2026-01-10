# Cleanup Task 014: Normalize block range semantics

## Goal
Make block range semantics consistent across architecture docs and contracts.

## Why
Right now the docs disagree about whether a range end is inclusive or end-exclusive. This creates ambiguity in:
- Deterministic dataset versioning (range identity).
- Partition keys and downstream materialization tracking.
- Operator docs that describe range splitting and compaction.

Because block ranges participate in identity (`config_hash`, dataset versioning, partition manifests), any disagreement here is effectively a contract bug.

## Plan
- Confirm the canonical range semantics from the current implementation and the owning spec.
- Establish one canonical statement for block range semantics (single source of truth).
- Update docs that currently contradict that canonical statement:
  - `docs/architecture/contracts.md` (partitioned event shape and `partition_key` description)
  - `docs/architecture/data_model/data_versioning.md` (partition key commentary)
  - `docs/specs/operators/range_aggregator.md`
  - `docs/specs/operators/range_splitter.md`
  - `docs/specs/operators/parquet_compact.md`
- Update the relevant spec if it is the source of the disagreement:
  - `docs/specs/chain_sync_entrypoint.md`
- Keep examples using `"1000000-1010000"` but define the semantics explicitly to avoid off-by-one ambiguity.

## Files to touch
- `docs/architecture/contracts.md`
- `docs/architecture/data_model/data_versioning.md`
- `docs/specs/operators/range_aggregator.md`
- `docs/specs/operators/range_splitter.md`
- `docs/specs/operators/parquet_compact.md`
- `docs/specs/chain_sync_entrypoint.md`

## Acceptance criteria
- No remaining contradictory claims about block range end semantics in the files listed above.
- Each doc that mentions a block range states the canonical semantics or links to the canonical statement.
- `partition_key` text does not contradict the numeric range fields.

## Suggested commit message
`docs: normalize block range semantics`
