# Trace Lite plan

This is the **agent-ready** build plan for **Trace Lite** (desktop/dev profile).

Lite is designed to be a *permanent* harness for correctness and demos:
- the **Dispatcher core and task lifecycle are identical** to AWS
- only infrastructure adapters differ (pgqueue vs SQS, MinIO vs S3)

## Lite profile definition

A minimal Lite stack is:

- Postgres (single instance)
  - **Postgres state**: orchestration truth (tasks, leases, outbox)
  - **Postgres data**: hot datasets (tables)
- MinIO (S3-compatible object store): cold Parquet + manifests
- Dispatcher (includes outbox publisher loop)
- Platform Worker (runs operators, including sinks)
- Query Service (DuckDB federation over Postgres data + Parquet)
- Optional:
  - RPC Egress Gateway (recommended if you use public RPC)
  - Delivery Service (optional for demo)

Lite uses the same **QueueDriver** abstraction as AWS:
- AWS: `QueueDriver = SQS`
- Lite: `QueueDriver = pgqueue`

## MVP checklist

MVP means: “a new developer can run one command and see an end-to-end pipeline succeed locally.”

### MVP-0: Compose boots

**Deliverable**
- `docker compose up` starts: Postgres, MinIO, Dispatcher, Platform Worker, Query Service

**Exit criteria**
- all services healthy
- migrations applied successfully (or an explicit bootstrap step exists)

### MVP-1: pgqueue driver works

**Deliverable**
- `QueueDriver` interface is implemented for:
  - publish
  - receive with visibility timeout
  - ack
  - dead-lettering after max attempts

**Exit criteria**
- publish → receive → ack works
- poison message reaches dead table after `max_attempts`

### MVP-2: Outbox → QueueDriver is the only enqueue path

**Deliverable**
- Dispatcher writes durable intent + outbox row in one transaction
- outbox publisher publishes via `QueueDriver`

**Exit criteria**
- you can stop the Dispatcher after creating tasks; on restart it resumes publishing outbox rows
- no code path exists that directly enqueues without an outbox row

### MVP-3: Worker lease lifecycle works

**Deliverable**
- worker consumes wake-up message(s), then claims task leases from Postgres state
- worker heartbeats and completes with strict attempt fencing

**Exit criteria**
- duplicate queue messages do not cause concurrent execution
- stale attempt completion cannot commit outputs

### MVP-4: Cold output commit protocol works

**Deliverable**
- worker writes Parquet to a staging prefix, then completes
- Dispatcher commits the pointer/manifest and emits downstream wake-ups

**Exit criteria**
- partial or abandoned staging outputs never become visible to reads
- retries do not produce multiple committed outputs for the same partition scope

### MVP-5: Demo DAG runs in bounded mode

**Deliverable**
- a demo DAG runs a bounded range (default) and produces:
  - at least one hot table (Postgres data)
  - at least one cold Parquet dataset (MinIO)
  - at least one Query Service result

**Exit criteria**
- “one command demo” prints a small result table or writes a result artifact
- default run completes in minutes (not hours)

Recommended default bounds:
- last **N blocks** (e.g., 10k–50k), configurable

### MVP-6: CI harness (recommended)

**Deliverable**
- CI runs the MVP demo under compose and asserts invariants

**Exit criteria**
- deterministic CI run
- asserts: no stuck tasks, no dead-letter messages, committed outputs exist

## Golden path demos

Lite should ship one demo that is fast by default, and optionally supports “full history.”

### Demo 1: Cryo sync + query across hot and cold

**Goal**
- show Cryo ingestion, hot + cold storage, and Query Service federation

**Default (bounded)**
- sync a bounded range for:
  - blocks
  - transactions
  - logs
  - (optional) traces if available and not too slow

**Query examples**
- “tip” query: recent blocks / tx count
- “backfill” query: aggregate across the bounded history range

**Output**
- hot tables in Postgres data
- cold Parquet dataset in MinIO
- query results returned inline or exported to MinIO depending on size

## Extended demos (Monad analytics)

These demos are intended to prove “real analytics value.” They may take longer and should be opt-in.

### Demo 2: Total supply by block (full history)

**Goal**
- compute total MONAD supply by block across the full chain history

**Notes**
- default Lite run should support “windowed history”
- full-history mode is opt-in

### Demo 3: Top N accounts and historic balances

**Goal**
- compute current top N accounts, then show their historic balances across time

### Demo 4: Staking and delegation concentration

**Goal**
- compute concentration metrics over staking + delegation relationships

## Performance knobs (Lite defaults)

Lite should prefer “fast and understandable” over maximum throughput.

Recommended knobs to expose:
- block range bounds (default bounded; full history opt-in)
- worker concurrency
- query result mode thresholds (inline size cap vs export)
- backpressure visibility (queue depth, oldest message age)

## Non-blocking nice-to-haves

- a `make demo` / `trace demo` wrapper that:
  - boots compose
  - bootstraps MinIO buckets
  - runs migrations
  - activates demo DAG
  - prints query results
- a small “echo webhook” container to demo Delivery Service
- docs page with “common gotchas” (MinIO creds, ports, disk space)
