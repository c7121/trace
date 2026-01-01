# Trace Lite plan

This document is the actionable implementation plan for **Trace Lite** (desktop/dev profile).
It is written for an engineer/agent to execute without needing AWS.

## Goals

- Run the core Trace architecture locally with **docker compose** in minutes.
- Keep the **Dispatcher core identical** between Lite and AWS:
  - Postgres state is truth (tasks, leases, outbox)
  - outbox publishes via `QueueDriver`
  - workers claim leases; queue is wake-up, not correctness
- Demonstrate a real end-to-end pipeline:
  - ingest via Cryo operators
  - write hot tables (Postgres data) and cold Parquet (MinIO)
  - query across hot+cold via Query Service
  - produce demonstrable analytics outputs for Monad-like chains

## Non-goals

- Security isolation: Lite is permissive and intended for local use.
- Production performance: pgqueue is not a high-throughput production queue.
- Full chain history by default: Lite should be runnable quickly; full-history runs are supported but not the default.

## No-rewrite rule (most important)

Lite must not introduce semantics that do not exist in AWS.

**Required:**
- Dispatcher writes durable intent + outbox row, then outbox publisher calls `QueueDriver.publish(...)`.
- Queue payloads remain small wake-up pointers (task_id, delivery_id, buffer record id).
- Task correctness remains enforced by Postgres state leases + strict attempt fencing.

**Avoid:**
- Special-casing Lite to enqueue directly inside the Dispatcher transaction.
- Depending on queue ordering or exactly-once semantics.

---

## Golden path demo: Cryo sync + query + analytics (Monad)

The demo is intentionally layered: it can run fast on a laptop using a bounded range, but supports extending to full history.

### Layer 1: Raw Cryo sync (datasets)

**Objective:** produce canonical raw datasets (hot + cold) that downstream queries can rely on.

Minimum datasets:
- blocks
- transactions
- logs
- traces (e.g., geth traces or transaction traces; pick one canonical trace format for v1 Lite)

Implementation notes:
- Use existing operators:
  - `cryo_ingest` for backfill ranges
  - `block_follower` for tip-follow (optional for v1 Lite; can be a looped range ingest)

Default demo parameters (fast):
- chain: configured RPC endpoint
- range: last N blocks (e.g., 50k) rather than genesis-to-tip
- concurrency: low defaults

### Layer 2: Query Service at tip and backfill

**Objective:** prove Query Service can answer:
- “tip” questions against hot tables
- “history” questions against cold parquet (or hot+cold federated)

Demonstrations:
- tip query: recent blocks/txs/logs
- backfill query: aggregate across a bounded history window

### Layer 3: Analytics demos (Monad-like)

These are the primary “wow” queries.

1) **Total supply by block across history**
- Input: transfers/mints/burns (depends on chain token model)
- Output: `{block_number, total_supply}` time series
- Demo mode: bounded history window; full-history mode optional

2) **Top N accounts**
- “Top N at tip” and their historic balances:
  - compute current top N holders
  - backfill balances for that fixed set across history window
- Output:
  - current leaderboard
  - timeseries balances for top N accounts

3) **Staking and delegation concentration**
- Input: staking/delegation events and/or validator stats datasets
- Output:
  - stake distribution by validator
  - delegation concentration metrics (e.g., Gini, top K share)

---

## Implementation plan (phased)

Each phase has an exit criterion. Phases are ordered to maximize reuse between Lite and AWS.

### Phase 0: Minimal Lite runtime and configuration

Deliverables:
- `docker-compose.lite.yml` (or compose profile) including:
  - Postgres (state + data)
  - MinIO
  - Dispatcher (with outbox publisher enabled)
  - Platform Worker
  - Query Service
  - Gateway (optional for demo UX)

Exit criteria:
- `docker compose up` brings all services to a healthy state
- migrations/bootstraps can run non-interactively

### Phase 1: QueueDriver interface and pgqueue implementation

Deliverables:
- `QueueDriver` interface used by:
  - outbox publisher
  - worker wake-ups
  - delivery work
  - buffered dataset sinks
- `PgQueueDriver` implementation + DDL/migrations

Exit criteria:
- can publish/receive/ack messages locally
- poison messages transition to a dead table after `max_attempts`

### Phase 2: Outbox publisher uses QueueDriver only

Deliverables:
- Dispatcher writes outbox rows for all queue side effects
- outbox publisher reads outbox and calls `QueueDriver.publish`

Exit criteria:
- creating a task produces an outbox row
- outbox publisher emits a wake-up message
- messages are idempotently published (no duplicates on retry beyond at-least-once)

### Phase 3: Worker consumption parity (Lite and AWS)

Deliverables:
- worker consumes wake-ups, claims task lease, runs, heartbeats, completes
- strict attempt fencing enforced at completion/commit boundary

Exit criteria:
- duplicate wake-ups do not cause concurrent execution
- stale attempts cannot commit outputs

### Phase 4: Object store parity (MinIO)

Deliverables:
- S3 client configured via endpoint for Lite
- bucket bootstrap (datasets/results/scratch)
- staging output + commit protocol works end-to-end

Exit criteria:
- tasks can stage Parquet outputs to MinIO and commit successfully
- uncommitted staging artifacts do not affect reads (they remain uncommitted)

### Phase 5: Demo DAG (Cryo sync + query service)

Deliverables:
- a demo DAG that:
  - backfills a bounded range into raw datasets
  - compacts into Parquet (cold)
  - runs a DuckDB query via Query Service and returns results

Exit criteria:
- demo completes on a laptop in a reasonable time window
- outputs are committed and query returns expected rows

### Phase 6: Analytics demos (Monad)

Deliverables:
- three demo queries packaged as:
  - stored SQL templates executed via Query Service, or
  - a small set of `duckdb_query` jobs

Exit criteria:
- produces tables for:
  - total supply by block
  - top N accounts at tip + historic balances
  - staking/delegation concentration

### Phase 7: Lite as regression harness

Deliverables:
- CI job runs compose, executes the demo DAG, asserts invariants

Exit criteria:
- Lite becomes the integration test for:
  - outbox correctness
  - lease/attempt correctness
  - commit protocol correctness
  - query service correctness

---

## Demo scope guidance (fast vs full)

To keep Lite usable:
- default demo should run a bounded window (e.g., last 50k blocks)
- full-history mode is opt-in via config

This lets you demonstrate correctness and value quickly without requiring hours of ingest.
