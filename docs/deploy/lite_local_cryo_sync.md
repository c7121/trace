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

Query Service:

```bash
cd crates/trace-query-service
cargo run
```

## 4) Apply a chain_sync job once (no planner loops)

Create a small `chain_sync` YAML job and apply it once. The Dispatcher runs a background planner tick and will
continuously top up in-flight work for the job until completion.

```bash
cat > /tmp/chain_sync.yaml <<'YAML'
kind: chain_sync
name: local_mainnet_bootstrap
chain_id: 1

mode:
  kind: fixed_target
  from_block: 0
  to_block: 1000 # syncs [0, 1000)

streams:
  blocks:
    cryo_dataset_name: blocks
    rpc_pool: standard
    chunk_size: 200
    max_inflight: 5
  geth_calls:
    cryo_dataset_name: geth_calls
    rpc_pool: traces
    chunk_size: 200
    max_inflight: 2
YAML

cd crates/trace-dispatcher
cargo run -- chain-sync apply --file /tmp/chain_sync.yaml
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
