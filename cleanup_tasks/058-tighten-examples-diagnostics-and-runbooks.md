# Cleanup Task 058: Tighten examples diagnostics and runbooks

## Goal

Make the diagnostics content under `docs/examples/` cohesive, correct, and low-drift by:
- making `docs/examples/data_verification.md` an index and entrypoint (link-first),
- eliminating incorrect bucket/path assumptions, and
- reducing duplicated SQL and DuckDB command snippets that already live with the runnable diagnostics.

## Why

Current issues:

- `docs/examples/data_verification.md` references a `trace-data` bucket and a `metadata.parquet` layout that does not exist in Trace Lite and is not referenced anywhere else.
- The runbook duplicates queries and DuckDB connection boilerplate that already exists in `harness/diagnostics/*`, which increases drift risk.
- `harness/diagnostics/*` READMEs link to `docs/examples/data_verification.md`, but the runbook only covers one diagnostic and does not help users discover the others.

## Plan

- Update `docs/examples/data_verification.md` to be link-first:
  - Add a short "Start here" section that explains prerequisites at a high level (harness running, data synced).
  - Replace the inline SQL + long `docker run ... duckdb` blocks with links to the runnable diagnostics:
    - `harness/diagnostics/duplicate_tx_detection/`
    - `harness/diagnostics/staking_withdrawal_verification/`
    - `harness/diagnostics/tvl_drop_detection/`
  - Remove the incorrect `trace-data` + `metadata.parquet` guidance and instead align to Trace Lite defaults (`S3_BUCKET=trace-harness`) by pointing readers to the diagnostics scripts that already encode the correct paths.
  - Keep TODO placeholders only if they have a clear planned owner; otherwise remove or replace with links to backlog/work items.

- Update `docs/examples/README.md` (small change):
  - Make the Diagnostics section explicitly point to `docs/examples/data_verification.md` as the entrypoint.
  - Optionally add a single link to `harness/diagnostics/` as the canonical home for runnable diagnostic scripts (avoid listing every diagnostic twice).

## Files to touch

- `docs/examples/README.md`
- `docs/examples/data_verification.md`
- (optional) `harness/diagnostics/*/README.md` only if link text needs to be updated for clarity (paths should remain stable).

## Acceptance criteria

- The docs no longer reference the nonexistent `trace-data` bucket or `metadata.parquet` layout.
- Diagnostics instructions live primarily with runnable scripts under `harness/diagnostics/` and the docs index links to them.
- The examples folder remains easy to navigate: a reader can find and run a diagnostic in 1-2 clicks.

## Suggested commit message

`docs: tighten diagnostics runbooks`
