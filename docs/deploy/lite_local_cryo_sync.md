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
cargo run --
```

## 4) Plan a small chain sync range

The planner schedules `cryo_ingest` tasks into the `task_wakeup` queue. `to_block` is exclusive.

```bash
cd crates/trace-dispatcher
cargo run -- plan-chain-sync --chain-id 1 --from-block 0 --to-block 1000 --chunk-size 200 --max-inflight 5
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

> Query Service is task-scoped (`POST /v1/task/query`) and only allows datasets granted in the task capability token.
> Wiring “dispatcher grants produced dataset versions to tasks” is part of the next milestone work.
