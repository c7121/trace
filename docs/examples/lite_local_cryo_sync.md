# Trace Lite local Cryo sync

This runbook proves Trace Lite end-to-end on a laptop:

- Apply a `chain_sync` YAML once.
- Dispatcher plans and schedules block ranges (genesis to tip-ish).
- `cryo_ingest` runs per range and publishes Parquet + a `_manifest.json`.
- Query Service attaches and scans Parquet remotely under a fail-closed SQL gate.

If you are doing this for the first time, use the `trace-lite` runner. Manual mode is kept as a troubleshooting fallback.

See also:
- [trace_lite.md](../plan/trace_lite.md) - `trace-lite` command reference
- [chain_sync_entrypoint.md](../specs/chain_sync_entrypoint.md) - chain_sync YAML semantics and payload contract

## Security and storage notes

- Query Service does not write Parquet to local disk. It resolves dataset manifests in trusted code, then DuckDB scans Parquet remotely.
  - Owners: [query_service.md](../architecture/containers/query_service.md) and [query_sql_gating.md](../specs/query_sql_gating.md)
  - Network posture: [ADR 0002](../adr/0002-networking.md)
- `cryo_ingest` stages Parquet locally before upload. Staging location, cleanup behavior, and artifact caps are owned by the operator spec:
  - [cryo_ingest.md](../specs/operators/cryo_ingest.md)
- If you need stronger local-staging guarantees, run the worker in a container with an ephemeral filesystem, or point `TMPDIR` at a dedicated encrypted/ephemeral mount.

## Prerequisites

- Docker + docker compose
- Rust toolchain
- Cryo binary (for real mode), or use `TRACE_CRYO_MODE=fake`
- RPC pool URLs for the pools referenced by your YAML (`TRACE_RPC_POOL_<POOL>_URL`)

## Recommended: use `trace-lite`

The `trace-lite` CLI starts the local dependencies (Postgres + MinIO) via Docker Compose, then runs the Rust services as local processes (dispatcher, sink, query service, cryo worker).

### 1) Start the stack

Terminal A:

```bash
export TRACE_CRYO_MODE=real
export TRACE_CRYO_BIN=/absolute/path/to/cryo

# At minimum, provide the pools referenced by your YAML.
export TRACE_RPC_POOL_STANDARD_URL='https://…'
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

For the YAML surface and semantics, see [chain_sync_entrypoint.md](../specs/chain_sync_entrypoint.md).

### 3) Watch progress

The `job_id` comes from your YAML (or from the output of `trace-lite apply`).

```bash
cargo run -p trace-lite -- status --job 4e20d260-8623-4e1c-a64a-9c4f4c8265d3
```

You should see per-stream cursor / scheduled range counts progressing. If a stream is blocked, the status output should include a reason.

### 4) Verify data

Run one of the runnable diagnostics in `harness/diagnostics/*`:
- [data_verification.md](data_verification.md)

### 5) Stop

- Ctrl-C in the `trace-lite up` terminal to stop the Rust processes.
- Optionally:

```bash
cargo run -p trace-lite -- down
```

(`down` stops the Docker services; it does not kill Rust processes that are already stopped by Ctrl-C.)

## Troubleshooting: manual mode

If you want to run each piece directly (useful when debugging), start from:
- [harness/README.md](../../harness/README.md)
- Harness "green" command: [AGENTS.md](../../AGENTS.md)

```bash
# Start Postgres + MinIO
cd harness && docker compose up -d

# Run migrations (state + data)
cargo run -p trace-harness -- migrate

# Start services (separate terminals)
cargo run -p trace-query-service
cargo run -p trace-harness -- dispatcher
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

## Common failure modes

- Missing `TRACE_RPC_POOL_<POOL>_URL`: follow-head planning cannot advance. The dispatcher logs a warning event `trace.dispatcher.chain_head_observer.missing_rpc_url`.
- Cryo exit code 2: treated as fatal (bad dataset name or invalid args), so the task fails without retrying.
- Artifact caps hit: the task fails as fatal. Reduce `chunk_size` in YAML or split ranges to keep per-range outputs smaller.
  - Owner: [cryo_ingest.md](../specs/operators/cryo_ingest.md)
