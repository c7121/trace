# Cleanup Task 059: Tighten Trace Lite end-to-end example guide

## Goal

Make `docs/examples/lite_local_cryo_sync.md` shorter and more link-first without losing important operational and security details:
- keep it actionable as an end-to-end "run Trace Lite" guide,
- move implementation details to canonical owner docs where appropriate, and
- reduce duplicated explanations that already exist in operator and service docs.

## Why

`docs/examples/lite_local_cryo_sync.md` is useful, but it repeats several implementation details that are already owned elsewhere:

- Cryo worker staging and artifact caps are described in the Cryo operator spec.
- Query Service remote Parquet scan behavior and safety controls are owned by Query Service docs and SQL gating docs.

Duplication increases drift risk, especially for default values and security-relevant behavior.

## Plan

- Update `docs/examples/lite_local_cryo_sync.md`:
  - Keep the `trace-lite` workflow as the primary path.
  - Keep manual mode as a troubleshooting fallback, but make it more link-first by pointing to:
    - harness README and green command for verification,
    - operator docs for Cryo behavior,
    - Query Service docs for scan behavior and safety model.
  - Replace detailed staging and cap descriptions with a short summary plus links to:
    - `docs/specs/operators/cryo_ingest.md`
    - `docs/architecture/containers/query_service.md`
    - `docs/specs/query_sql_gating.md`
  - Add a direct link to `docs/specs/chain_sync_entrypoint.md` for the chain_sync YAML surface, and keep the example YAML link.

## Files to touch

- `docs/examples/lite_local_cryo_sync.md`
- (optional, if needed to preserve information without duplication) `docs/specs/operators/cryo_ingest.md`

## Acceptance criteria

- The end-to-end guide remains runnable and clear.
- No unique operational or security information is dropped; anything removed from the example is moved into a more appropriate owner doc and linked.
- The example becomes shorter and less repetitive.

## Suggested commit message

`docs: tighten trace lite example guide`
