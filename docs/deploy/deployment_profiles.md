# Deployment Profiles

Trace supports two deployment profiles:

- **AWS (production)**: S3 + SQS + Postgres state + Postgres data.
- **Trace Lite (desktop/dev)**: MinIO + pgqueue + Postgres while preserving the same orchestration semantics.

The design intent is that **Dispatcher core logic and task lifecycle behavior are identical** between profiles, to avoid codepath drift.

## Non-goals

- Trace Lite is not a security sandbox. Auth and permissions may be permissive.
- Lite does not attempt IAM/STS/VPC parity.

## Core invariants

These invariants are required in **all** profiles:

1. **Postgres state is the source of truth** for orchestration:
   - tasks, attempts, leases, heartbeats
   - outbox entries
   - dataset commit metadata and active pointers
2. **Queue delivery is at-least-once**:
   - duplicates may occur
   - ordering is not guaranteed
   - correctness must not depend on FIFO
3. **Queue messages are wake-ups, not authority**:
   - workers must claim work via Postgres state leasing
4. **Strict attempt fencing**:
   - any output commit includes `(task_id, attempt)`
   - the Dispatcher rejects commits for stale attempts
5. **Outbox is required**:
   - durable intent + outbox row are written in the same DB transaction
   - an outbox publisher executes the side effect (enqueue) later

## Adapter matrix

| Capability | AWS | Trace Lite |
|---|---|---|
| Object store (cold Parquet) | S3 | MinIO (S3-compatible) |
| Queue backend | SQS (Standard) | pgqueue (Postgres-backed queue) |
| Postgres state | RDS Postgres | Postgres container (db/schema) |
| Postgres data | RDS Postgres | Postgres container (db/schema) |
| Cron triggers | EventBridge Scheduler/Rules -> Lambda | compose `scheduler` container or local cron -> HTTP |
| Webhooks ingress | API Gateway -> Lambda or Gateway | Gateway HTTP directly |

## Strict rule: Dispatcher core is identical

**Normative requirement:** The Dispatcher must never enqueue directly as part of creating tasks or buffered work.

The only allowed flow is:

1) write durable intent (task row / buffer record row)
2) write an outbox row describing the enqueue
3) commit
4) outbox publisher later calls `QueueDriver.publish(...)`

This applies even in Trace Lite where pgqueue lives in Postgres.
Do not optimize Lite by inserting into `queue_messages` in the same transaction as task creation.

Publisher implementation note:
- the outbox publisher may run as a separate process/container or as a loop inside the Dispatcher
- outbox rows must be marked as sent in a separate transaction from the intent-creation transaction

## One queue abstraction

All internal queue use cases use the same QueueDriver interface (task wake-ups, buffered datasets, delivery work).

### QueueDriver operations

Required operations:

- `publish(queue_name, payload_json, delay_seconds=0)`
- `receive(queue_name, max_messages, visibility_timeout_seconds) -> [Message]`
- `ack(queue_name, receipt)`

Recommended:

- `extend_visibility(queue_name, receipt, visibility_timeout_seconds)`

Where `Message` includes:

- `payload_json`
- `receipt` (opaque handle used for ack/extend)
- `delivery_count` (best-effort)

### Required semantics

- at-least-once delivery
- duplicates allowed
- no ordering guarantee
- visibility timeout supported

Poison handling:

- AWS: SQS redrive policy to DLQ
- Lite: pgqueue `max_attempts` moved to dead table

## Queue payload shapes

Keep payloads small and typed:

- Task wake-up: `{"kind":"task_wakeup","task_id":"<uuid>"}`
- Buffered record: `{"kind":"buffer_record","dataset_uuid":"<uuid>","record_id":"<uuid>"}`
- Delivery work: `{"kind":"delivery","delivery_id":"<uuid>"}`

## pgqueue backend (Trace Lite)

pgqueue is a Postgres-backed QueueDriver used for desktop installs, CI, and demos.
It is not intended as the primary production queue.

### Minimal DDL

```sql
CREATE TABLE queue_messages (
  id            BIGSERIAL PRIMARY KEY,
  queue_name    TEXT NOT NULL,
  payload       JSONB NOT NULL,

  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  visible_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

  lease_until   TIMESTAMPTZ,
  lease_token   UUID,

  attempts      INT NOT NULL DEFAULT 0,
  max_attempts  INT NOT NULL DEFAULT 20,

  last_error    TEXT
);

CREATE INDEX queue_ready_idx
  ON queue_messages (queue_name, visible_at, id);

CREATE INDEX queue_lease_idx
  ON queue_messages (queue_name, lease_until);

CREATE TABLE queue_dead (
  id           BIGINT PRIMARY KEY,
  queue_name   TEXT NOT NULL,
  payload      JSONB NOT NULL,
  created_at   TIMESTAMPTZ NOT NULL,
  dead_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  attempts     INT NOT NULL,
  last_error   TEXT
);
```

### Receive and lease algorithm (conceptual)

Select available rows and lock:

```sql
SELECT id, payload
FROM queue_messages
WHERE queue_name = $1
  AND visible_at <= now()
  AND (lease_until IS NULL OR lease_until < now())
  AND attempts < max_attempts
ORDER BY id
LIMIT $2
FOR UPDATE SKIP LOCKED;
```

Lease the selected rows:

```sql
UPDATE queue_messages
SET lease_until = now() + ($3 || ' seconds')::interval,
    lease_token = gen_random_uuid(),
    attempts = attempts + 1
WHERE id = ANY($ids)
RETURNING id, payload, lease_token, attempts;
```

Return `receipt = {id, lease_token}`.

### Ack

```sql
DELETE FROM queue_messages
WHERE id = $1 AND lease_token = $2;
```

### Dead-lettering

```sql
WITH moved AS (
  DELETE FROM queue_messages
  WHERE attempts >= max_attempts
  RETURNING id, queue_name, payload, created_at, attempts, last_error
)
INSERT INTO queue_dead (id, queue_name, payload, created_at, attempts, last_error)
SELECT id, queue_name, payload, created_at, attempts, last_error FROM moved;
```

## Trace Lite docker compose

A minimal Lite stack should be runnable with `docker compose up` and include:

- `postgres` (state + data)
- `minio` (S3-compatible object store)
- `gateway`
- `dispatcher` (runs the outbox publisher loop, or runs alongside a separate publisher)
- `query_service`
- `platform_worker`
- optional `delivery_service` (webhook demo)
- optional `rpc_egress_gateway` (RPC calls for demo)
- optional `scheduler` (cron-like triggers for demo DAGs)

## Triggers

### Trace Lite

- **Cron**: a `scheduler` container (or local cron) that calls a Dispatcher endpoint.
- **Webhook**: the Gateway exposes HTTP endpoints directly.

Both must translate to the same internal behavior:

- write durable intent + outbox row in Postgres state
- outbox publisher enqueues wake-ups via QueueDriver

### AWS

- **Cron**: EventBridge Scheduler/Rules -> Lambda -> Dispatcher enqueue
- **Webhook**: API Gateway -> Lambda (or Gateway) -> Dispatcher enqueue

The Dispatcher core remains unchanged; only the trigger adapter differs.
## Development path forward (Lite first, no rewrite later)

Trace Lite should not be a separate architecture. It should be the same orchestration core with different infrastructure adapters.

To avoid drift:

- Keep the Dispatcher state machine identical across profiles (leases, attempts, outbox, strict attempt fencing).
- Keep the enqueue path identical across profiles: durable intent + outbox row in Postgres state, then an outbox publisher calls `QueueDriver.publish`.
- Keep queue payloads identical across profiles (wake-up pointers, not task payloads).
- Swap only adapter implementations:
  - `QueueDriver=sqs` in AWS vs `QueueDriver=pgqueue` in Lite
  - `ObjectStore=s3` in AWS vs `ObjectStore=minio` in Lite (S3-compatible API)
- Lite auth may be permissive, but the endpoint shapes and request flows must remain the same.

If you follow these rules, Lite becomes a permanent integration harness: you can develop the full system while continuously validating core invariants (rehydration, attempt fencing, outbox safety) without needing AWS.

## Implementation plan: build Trace Lite

This plan is written so an engineer or agent can execute it with minimal back-and-forth. Each phase has an exit criterion.

### Phase 0: Minimal Lite runtime and configuration

- [ ] Decide the default demo chain and range (example: Monad mainnet, last 50,000 blocks, overridable).
- [ ] Add a `TRACE_PROFILE=lite|aws` (or equivalent) config that selects:
  - queue driver: `pgqueue` (lite) or `sqs` (aws)
  - object store endpoint: MinIO endpoint for lite, AWS default for aws
  - state/data DSNs
- [ ] Add Lite docker compose services (minimum):
  - postgres
  - minio
  - dispatcher (includes outbox publisher loop)
  - platform worker
  - query service

**Exit:** `docker compose up` starts services and health checks pass.

### Phase 1: QueueDriver interface and pgqueue implementation

- [ ] Define a QueueDriver interface used everywhere the system enqueues/consumes internal work:
  - task wake-ups
  - buffered dataset records
  - delivery work
- [ ] Implement `pgqueue` driver using Postgres:
  - `publish(queue_name, payload_json, delay_seconds=0)`
  - `receive(queue_name, max_messages, visibility_timeout_seconds)`
  - `ack(queue_name, receipt)`
  - optional `extend_visibility(...)`
- [ ] Create schema/migration for `queue_messages` and `queue_dead` tables.

**Exit:** a standalone smoke test can publish, receive, ack, and dead-letter messages.

### Phase 2: Outbox publisher uses QueueDriver only

- [ ] Ensure the Dispatcher never enqueues directly.
- [ ] Ensure all enqueue side effects are emitted as outbox rows inside the same transaction as the durable intent.
- [ ] Run an outbox publisher loop that:
  - reads outbox rows
  - calls `QueueDriver.publish`
  - marks rows sent

**Exit:** creating a task row produces a visible wake-up message in the queue backend.

### Phase 3: Worker consumption (Lite + AWS parity)

- [ ] Update platform workers to consume from QueueDriver (not directly from SQS SDK).
- [ ] Worker flow remains: wake-up -> claim lease -> heartbeat -> complete.
- [ ] Extend visibility during long task runs (optional in lite, required in aws).

**Exit:** tasks run end-to-end in Lite with retries, and no stale attempt can commit outputs.

### Phase 4: Object store parity (MinIO)

- [ ] Configure the S3 client to work with:
  - AWS S3 in prod
  - MinIO (S3-compatible) in Lite
- [ ] Validate multipart upload and list operations used by compaction and query export.
- [ ] Add bucket bootstrap step in Lite (create buckets at startup if missing).

**Exit:** platform worker can stage and commit Parquet outputs to MinIO; query service can export results to MinIO.

### Phase 5: Lite demo DAG (Cryo sync + query)

Goal: demonstrate the system end-to-end using your actual use cases, but with a demo-friendly default scope.

- [ ] Create an example DAG that runs in Lite and exercises:
  - Cryo backfill for core datasets (blocks, transactions, logs, traces)
  - Tip follow updates (optional for v0; backfill only is acceptable initially)
  - Query service reads across hot (Postgres data) and cold (Parquet)
- [ ] Provide a single command demo entry point:
  - `make demo` or `trace demo`
  - prints where outputs were written and runs a query that returns a visible result

**Exit:** a new developer can run one command and see results without AWS credentials.

### Phase 6: Demo queries (Monad analytics)

Use Query Service to demonstrate:

- [ ] Total supply by block across the chosen range:
  - recommended approach: compute supply deltas from ERC20 Transfer mints and burns, then cumulative sum
  - (chain-specific contract address and ABI are config)
- [ ] Top N accounts and historic balances:
  - recommended approach: compute current top N at tip, then compute historic balances for those accounts only (filter transfers involving those accounts)
- [ ] Staking and delegation concentration:
  - ingest staking events, derive concentration metrics per epoch or block range

**Exit:** the demo prints 2–3 tables (or exports) that match the above use cases.

### Phase 7: Keep Lite as your regression harness

- [ ] Add CI workflow that runs the Lite compose stack and executes the golden path DAG.
- [ ] CI asserts the invariants:
  - no stuck Running tasks with expired leases
  - no stale attempt commits
  - outbox drains to zero
  - queue dead-letter table is empty (or contains only expected poison cases)

**Exit:** Lite run passes in CI and becomes the default platform regression test.

## Golden path demo (proposed structure)

Your demo use cases are large. The recommended approach is to implement them in layers so the demo is fast by default, but can be scaled up by changing parameters.

### Layer 1: Raw Cryo sync

Datasets (minimum):

- blocks
- transactions
- logs
- traces (geth_trace or equivalent)

Outputs:

- cold Parquet partitions in the object store
- a small hot table in Postgres data for the most recent window (configurable)

### Layer 2: Tip + backfill query

Demonstrate that Query Service can query:

- latest state (tip window) from Postgres data
- historical data from Parquet
- joins across hot and cold (DuckDB federation)

### Layer 3: Analytics demos

- total supply by block across the range
- top N accounts at tip and their historic balances across the range
- staking and delegation concentration across the range

Implementation note: for Lite performance, default to a bounded block range and make the range configurable.
### Example query sketches

These are intentionally schematic. They are meant to be runnable with minor adaptation once dataset names and columns are finalized.

#### Total supply by block (ERC20-style mints and burns)

Assumes a transfer dataset with columns: `token_address`, `from_address`, `to_address`, `value`, `block_number`.

Compute per-block supply deltas and cumulative supply:

```sql
WITH deltas AS (
  SELECT
    block_number,
    SUM(
      CASE
        WHEN from_address = '0x0000000000000000000000000000000000000000' THEN value
        WHEN to_address   = '0x0000000000000000000000000000000000000000' THEN -value
        ELSE 0
      END
    ) AS supply_delta
  FROM erc20_transfers
  WHERE token_address = :token_address
  GROUP BY 1
),
supply AS (
  SELECT
    block_number,
    SUM(supply_delta) OVER (ORDER BY block_number ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS total_supply
  FROM deltas
)
SELECT * FROM supply ORDER BY block_number;
```

If Monad total supply is not ERC20-style, replace the delta source with the chain-specific supply signal (for example, staking contract events, mint events, or a dedicated supply contract).

#### Top N accounts at tip, and their historic balances

Step 1: compute balances at tip across a bounded range:

```sql
WITH flows AS (
  SELECT
    CASE WHEN to_address = account THEN value ELSE 0 END
    - CASE WHEN from_address = account THEN value ELSE 0 END
    AS delta,
    block_number,
    account
  FROM (
    SELECT to_address AS account, value, block_number, from_address, to_address FROM erc20_transfers WHERE token_address = :token_address
    UNION ALL
    SELECT from_address AS account, value, block_number, from_address, to_address FROM erc20_transfers WHERE token_address = :token_address
  )
),
balances AS (
  SELECT account, SUM(delta) AS balance
  FROM flows
  GROUP BY 1
),
topn AS (
  SELECT account
  FROM balances
  ORDER BY balance DESC
  LIMIT :n
)
SELECT * FROM balances WHERE account IN (SELECT account FROM topn) ORDER BY balance DESC;
```

Step 2: compute historic balances for only those accounts:

```sql
WITH topn AS (
  SELECT account FROM top_accounts_at_tip
),
deltas AS (
  SELECT
    block_number,
    account,
    SUM(delta) AS delta
  FROM (
    SELECT block_number, to_address AS account, value AS delta
    FROM erc20_transfers
    WHERE token_address = :token_address AND to_address IN (SELECT account FROM topn)
    UNION ALL
    SELECT block_number, from_address AS account, -value AS delta
    FROM erc20_transfers
    WHERE token_address = :token_address AND from_address IN (SELECT account FROM topn)
  )
  GROUP BY 1, 2
),
hist AS (
  SELECT
    block_number,
    account,
    SUM(delta) OVER (PARTITION BY account ORDER BY block_number) AS balance
  FROM deltas
)
SELECT * FROM hist ORDER BY block_number, account;
```

#### Staking and delegation concentration

Assume a normalized staking dataset: `staking_events(validator, delegator, amount_delta, block_number)` where `amount_delta` is positive for stake, negative for unstake.

Example: concentration by validator at a given block window:

```sql
WITH by_validator AS (
  SELECT validator, SUM(amount_delta) AS staked
  FROM staking_events
  WHERE block_number BETWEEN :start_block AND :end_block
  GROUP BY 1
),
totals AS (
  SELECT SUM(staked) AS total_staked FROM by_validator
),
ranked AS (
  SELECT
    validator,
    staked,
    staked / NULLIF((SELECT total_staked FROM totals), 0) AS share,
    RANK() OVER (ORDER BY staked DESC) AS rnk
  FROM by_validator
)
SELECT * FROM ranked ORDER BY staked DESC;
```

