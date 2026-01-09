# Lite local Cryo sync

This runbook is for **proving Trace Lite end-to-end** on a laptop:

- You apply a `chain_sync` YAML once.
- The dispatcher schedules ranges (genesis → tip-ish).
- Cryo runs per range and publishes Parquet + a `_manifest.json`.
- Query Service scans the Parquet **in-place** (remote scan) and only fetches `_manifest.json`.

If you’re doing this for the first time, use the `trace-lite` runner. The older manual steps are kept as a troubleshooting fallback.

## Security note about Parquet on disk

- **Query Service does not download Parquet objects**; it only fetches `_manifest.json`, then DuckDB scans remote Parquet via `httpfs`.
- **The Cryo worker does write Parquet locally**, but only into a **temporary staging directory** and then uploads to the object store.
  - On success, the staging dir is deleted.
  - On crashes, temp dirs may remain (standard caveat).

If you need stronger guarantees, run the worker in a container with an ephemeral filesystem, or point `TMPDIR` at a dedicated encrypted/ephemeral mount.

## Prerequisites

- Docker
- Rust toolchain
- A Cryo binary available locally (recommended), or built from the Cryo repo
- RPC URLs for the networks you want to sync

Environment variables you’ll commonly set:

- `CRYO_BIN=/path/to/cryo` (or ensure `cryo` is on `PATH`)
- `TRACE_CRYO_MODE=fake|real` (default is fake; use `real` for actual sync)
- RPC pool URLs (choose names that match your YAML `rpc_pool` fields):
  - `TRACE_RPC_POOL_DEFAULT_URL=...`
  - `TRACE_RPC_POOL_TRACES_URL=...` (if you want traces on a separate endpoint)

## Recommended: use `trace-lite`

The `trace-lite` CLI starts the local dependencies (Postgres + MinIO) via Docker Compose, then runs the Rust services as local processes (dispatcher, sink, query service, cryo worker).

### 1) Start the stack

Terminal A:

```bash
export TRACE_CRYO_MODE=real
export CRYO_BIN=/absolute/path/to/cryo

# At minimum, provide the pools referenced by your YAML.
export TRACE_RPC_POOL_DEFAULT_URL='https://…'
export TRACE_RPC_POOL_TRACES_URL='https://…'

cargo run -p trace-lite -- up
```

Leave this running. `trace-lite up` stays in the foreground; press Ctrl-C to stop the Rust processes.

### 2) Apply a chain sync job

Terminal B:

```bash
cargo run -p trace-lite -- apply --file docs/examples/chain_sync.monad_mainnet.yaml
```

You can apply additional jobs later (different YAML files). Apply is idempotent.

### 3) Watch progress

The `job_id` comes from your YAML (or from the output of `trace-lite apply`).

```bash
cargo run -p trace-lite -- status --job 4e20d260-8623-4e1c-a64a-9c4f4c8265d3
```

You should see per-stream cursor / scheduled range counts progressing. If a stream is blocked, the status output should include a reason.

### 4) Stop

- Ctrl-C in the `trace-lite up` terminal to stop the Rust processes.
- Optionally:

```bash
cargo run -p trace-lite -- down
```

(`down` stops the Docker services; it does not kill Rust processes that are already stopped by Ctrl-C.)

## Troubleshooting: manual mode

If you want to run each piece directly (useful when debugging):

```bash
# Start Postgres + MinIO
cd harness && docker compose up -d

# Start services (separate terminals)
cargo run -p trace-query-service
cargo run -p trace-dispatcher -- serve
cargo run -p trace-harness -- sink
cargo run -p trace-harness -- cryo-worker

# Apply job
cargo run -p trace-dispatcher -- apply --file docs/examples/chain_sync.monad_mainnet.yaml

# Status
cargo run -p trace-dispatcher -- status --job 4e20d260-8623-4e1c-a64a-9c4f4c8265d3
```

When in doubt, run the test suite (it encodes the invariants we care about):

```bash
(cd harness && cargo test -- --nocapture)
```
