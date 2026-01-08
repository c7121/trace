# Lite local Cryo sync runbook

This runbook proves the Lite stack can:
- schedule `cryo_ingest` range tasks (planner),
- execute them locally (worker), and
- register Parquet dataset versions in Postgres **state** for later task-scoped querying.

## Prereqs
- Rust stable
- Docker + `docker compose`
- (Optional, real mode) `cryo` CLI installed

## 1) Start dependencies

```bash
cd harness
docker compose up -d
```

## 2) Run migrations

```bash
cd harness
cargo run -- migrate
```

## 3) Start services (separate terminals)

Dispatcher:

```bash
cd harness
cargo run -- dispatcher
```

For `follow_head` jobs, the Dispatcher must be able to poll `eth_blockNumber` for each `rpc_pool` name referenced by the job.
Provide pool RPC URLs via env vars such as `TRACE_RPC_POOL_STANDARD_URL` and `TRACE_RPC_POOL_TRACES_URL`.

Query Service:

```bash
cd crates/trace-query-service
cargo run
```

## 4) Apply a chain_sync job once (no planner loops)

Use the committed example at `docs/examples/chain_sync.ethereum_mainnet.yaml` and apply it once. The Dispatcher
runs a background planner tick and will continuously top up in-flight work for the job until completion.

```bash
cd crates/trace-dispatcher
cargo run -- apply --file ../../docs/examples/chain_sync.ethereum_mainnet.yaml
```

## 5) Run the Cryo worker

Fake mode (default; deterministic Parquet for dev/tests):

```bash
cd harness
cargo run -- cryo-worker
```

Real mode (runs `cryo` CLI; opt-in):

```bash
cd harness
TRACE_CRYO_MODE=real \\
TRACE_CRYO_RPC_URL="http://localhost:8545" \\
cargo run -- cryo-worker
```

If you use multiple `rpc_pool` values in a job, set per-pool RPC URLs instead:
- `TRACE_RPC_POOL_STANDARD_URL`
- `TRACE_RPC_POOL_TRACES_URL`

## 6) Verify state

Inspect the dataset version registry:

```bash
psql "postgres://trace:trace@localhost:5433/trace_state" -c "select dataset_uuid, dataset_version, storage_prefix, storage_glob, range_start, range_end, config_hash from state.dataset_versions order by created_at desc limit 20;"
```

Optional: inspect chain sync progress (per-stream cursors and scheduled ranges):

```bash
psql "postgres://trace:trace@localhost:5433/trace_state" -c "select * from state.chain_sync_jobs order by updated_at desc limit 10;"
psql "postgres://trace:trace@localhost:5433/trace_state" -c "select * from state.chain_sync_cursor order by updated_at desc limit 20;"
psql "postgres://trace:trace@localhost:5433/trace_state" -c "select job_id, dataset_key, range_start, range_end, status from state.chain_sync_scheduled_ranges order by created_at desc limit 50;"
```

> Query Service is task-scoped (`POST /v1/task/query`) and only allows datasets granted in the task capability token.
