# trace-lite

`trace-lite` is a **dev-only local stack runner**. It exists to eliminate the "write a shell script" phase
and make Trace Lite usable as a single coherent system.

It is intentionally simple:

- Docker Compose runs the *dependencies* (Postgres + MinIO).
- Rust processes run the *services* (dispatcher, Query Service, sink, and cryo worker).
- Chain sync jobs are defined in YAML and applied once; the dispatcher schedules until done.

## Commands

Run from the repo root.

Start the local stack (foreground; Ctrl-C stops Rust processes):

```bash
cargo run -p trace-lite -- up
```

Apply a chain sync job YAML:

```bash
cargo run -p trace-lite -- apply --file docs/examples/chain_sync.monad_mainnet.yaml
```

Check progress:

```bash
# List all jobs:
cargo run -p trace-lite -- status

# Or filter to a single job_id from your YAML (or from the output of `trace-lite apply`):
cargo run -p trace-lite -- status --job 4e20d260-8623-4e1c-a64a-9c4f4c8265d3
```

Stop Docker dependencies:

```bash
cargo run -p trace-lite -- down
```

## Environment

At minimum you usually want:

- `TRACE_RPC_POOL_<POOL>_URL` for any RPC pool referenced by your chain-sync YAML
  (e.g. `rpc_pool: standard` -> `TRACE_RPC_POOL_STANDARD_URL`).
- `TRACE_CRYO_MODE=real` if you want to run the real Cryo binary.
- `TRACE_CRYO_BIN=/path/to/cryo` if the `cryo` binary is not already on your `PATH`.

See the full end-to-end runbook:

- `docs/examples/lite_local_cryo_sync.md`

## Notes

- Query Service scans Parquet **in place** (remote scan) and fetches only `_manifest.json`.
- For any real deployment, you must enforce a **host/container egress allowlist** so a compromised
  Query Service cannot reach arbitrary network destinations.
