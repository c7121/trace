# cryo_ingest

Bootstrap historical onchain data using [Cryo](https://github.com/paradigmxyz/cryo).

Status: implemented (Lite)

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_platform` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerPartition |
| **Image** | `cryo_ingest:latest` |

## Contract

This operator typically runs as a `chain_sync`-planned task that executes Cryo for one dataset and publishes exactly one dataset version.

- Range semantics: `[range_start, range_end)` (end-exclusive). Invoke Cryo as `--blocks {range_start}:{range_end}` (do not pre-decrement).
- Single-output: one successful attempt publishes exactly one `DatasetPublication`.

### Task payload

These fields are the canonical `chain_sync`-planned payload contract.

| Field | Type | Description |
|-------|------|-------------|
| `dataset_uuid` | event | Published dataset identity for this stream (resolved at apply-time) |
| `dataset_key` | event | Stable stream key from config (debugging and validation) |
| `chain_id` | event | Target chain ID |
| `cryo_dataset_name` | event | Cryo dataset name (first CLI arg), for example `blocks` or `logs` |
| `rpc_pool` | event | RPC pool name for the worker to resolve to a URL (secrets must not appear in YAML) |
| `range_start` | event | Inclusive start block |
| `range_end` | event | End-exclusive end block |
| `config_hash` | event | Configuration hash used for deterministic `dataset_version` derivation |

### Output publication

The worker publishes one dataset version:

- `DatasetPublication`: `{ dataset_uuid, dataset_version, storage_ref, config_hash, range_start, range_end }`
- `storage_ref` (harness layout): `s3://{bucket}/cryo/{chain_id}/{dataset_uuid}/{range_start}_{range_end}/{dataset_version}/`
  - All `.parquet` files produced by Cryo are uploaded under this prefix.
  - A `_manifest.json` is written under the same prefix listing the uploaded Parquet object keys.

## Implementation notes

- Harness defaults to a deterministic stub (`TRACE_CRYO_MODE=fake`) so tests do not require the real Cryo binary or chain RPC access.
- Real Cryo mode (`TRACE_CRYO_MODE=real`) requires:
  - `TRACE_CRYO_BIN` (optional, default `cryo`)
  - `TRACE_CRYO_RPC_URL` or `TRACE_RPC_POOL_<NAME>_URL` (resolved from `rpc_pool`)
- Local staging:
  - writes to `/tmp/trace/cryo/<task_id>/<attempt>/` with private permissions
  - deletes the staging directory after successful upload
  - deletes stale staging dirs on startup based on `TRACE_CRYO_STAGING_TTL_HOURS` (default `24`)
- Parquet safety caps (env, optional): `MAX_PARQUET_FILES_PER_RANGE`, `MAX_PARQUET_BYTES_PER_FILE`, `MAX_TOTAL_PARQUET_BYTES_PER_RANGE`
- Future work: wrap Cryo as a Rust library and stream Parquet objects directly to the object store without local staging (track in `docs/plan/backlog.md`).

## Related

- `chain_sync` planning and payload contract: [chain_sync_entrypoint.md](../chain_sync_entrypoint.md)
- Example chain_sync config: [chain_sync.monad_mainnet.yaml](../../examples/chain_sync.monad_mainnet.yaml)
- Task-scoped dataset event contract: [task_scoped_endpoints.md](../../architecture/contracts/task_scoped_endpoints.md)
- Harness implementation: [harness/src/cryo_worker.rs](../../../harness/src/cryo_worker.rs)
