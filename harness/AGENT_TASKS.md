# Harness implementation tasks (for agents)

This file is written for an implementation agent (e.g., Codex) to build the contract-freeze harness
**inside this repo** using idiomatic Rust.

The goal is to prove the **core platform invariants** under failure/duplicates, *not* to build all features.

## Idiomatic Rust expectations

- Use `tokio` for async, `axum` for HTTP, `sqlx` for Postgres.
- Prefer typed structs + enums (domain types) over ad-hoc `serde_json::Value`.
- No `unwrap()` / `expect()` in production paths; use `anyhow::Context` for errors.
- Use `tracing` (already wired) for structured logs.
- Keep modules small and cohesive; avoid “god modules”.
- Write integration tests that are deterministic and assert invariants.

## Recommended commit structure (easy review)

1. `chore(harness): add skeleton + compose + migrations` (already done)
2. `feat(harness): pgqueue adapter (receive/ack/publish + visibility)`
3. `feat(harness): dispatcher core (lease claim, fencing, outbox drain loop)`
4. `feat(harness): worker wrapper happy path (claim → publish → complete)`
5. `feat(harness): sink consumer (validate → idempotent upsert → dlq)`
6. `test(harness): integration tests for duplicates + stale fencing + crash recovery`

Keep commits small enough that each has a clear pass/fail verification step.

## Verification commands (every commit)

From `harness/`:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

For runtime verification (as you implement components):

```bash
docker compose up -d
cargo run -- migrate
cargo run -- dispatcher
cargo run -- worker
cargo run -- sink
```

## Task checklist (build in this order)

### Task 0: Baseline compile + migrations (should already pass)
- [ ] `docker compose up -d`
- [ ] `cargo run -- migrate` succeeds
- [ ] Confirm tables exist in both DBs (`state.tasks`, `state.queue_messages`, `data.alert_events`)

### Task 1: pgqueue adapter (state DB)
Implement a small queue module:
- `publish(queue_name, payload, available_at)`
- `receive(queue_name, max, visibility_timeout)` using `FOR UPDATE SKIP LOCKED`
- `ack(message_id)`
- (optional) `extend_visibility(message_id, invisible_until)`

**Verification**
- [ ] unit test that `publish` then `receive` returns the message
- [ ] `receive` twice with same queue returns message only once until visibility expires
- [ ] after `ack`, message never reappears

### Task 2: Dispatcher minimal HTTP API
Implement only these endpoints:

#### `POST /internal/task-claim` (trusted worker-only)
Input: `{ "task_id": "<uuid>" }`
Output: `{ "task_id", "attempt", "lease_token", "lease_expires_at", "capability_token", "payload" }`

Behavior:
- If task is `queued`, set `status=running`, set `lease_token`, `lease_expires_at=now+lease_duration`.
- If task is `running` with unexpired lease, reject claim (409).
- If lease expired, bump attempt and re-lease (or schedule retry first; harness can bump directly).

#### `POST /v1/task/heartbeat` (untrusted)
Requires:
- `X-Trace-Task-Capability: <capability_token>`
- Body contains `{task_id, attempt, lease_token}`
Behavior:
- Only current attempt + matching lease_token may extend lease.

#### `POST /v1/task/buffer-publish` (untrusted)
Same auth + fencing requirements.
Body includes `{ batch_uri, content_type, batch_size_bytes }`.
Behavior:
- Insert outbox record that will publish `{batch_uri,...}` to `buffer_queue`.

#### `POST /v1/task/complete` (untrusted)
Same auth + fencing.
Behavior:
- Mark task succeeded (idempotent).
- Insert outbox record for downstream routing (can be stubbed in harness).

Background loops:
- outbox drainer: `pending` → publish to queue → mark `sent` (at-least-once allowed)

**Verification**
- [ ] can claim a seeded task
- [ ] heartbeat extends lease
- [ ] buffer-publish enqueues a message via outbox
- [ ] completion is idempotent (calling twice doesn’t break)

### Task 3: Capability tokens (JWT) — Phase 2
Start without JWT (just lease fencing), then add JWT:
- Dispatcher issues a signed token that matches the **capability token claim contract** in `docs/architecture/contracts.md`
  (standard claims + required `{org_id, task_id, attempt}` and scopes).
- `/v1/task/*` verifies signature + required claims (`iss/aud/exp/kid`) and binds `{task_id, attempt}` in the body to token claims.

**Verification**
- [ ] invalid token rejected
- [ ] token for task A cannot be used for task B

### Task 4: Worker wrapper happy path
Implement a worker loop:
- poll `task_wakeup` queue
- claim via dispatcher
- write a **valid JSONL** alert batch artifact to object store (MinIO) OR to local filesystem initially
- call buffer-publish
- call complete

**Verification**
- [ ] 1 seeded task produces 1 buffer message
- [ ] duplicate wakeups do not produce double inserts (later verified at sink)

### Task 5: Sink consumer
Poll `buffer_queue`:
- fetch `batch_uri`
- parse JSONL
- strict schema validation
- upsert to `data.alert_events` with `ON CONFLICT (dedupe_key) DO NOTHING`
- on repeated failure → DLQ

**Verification**
- [ ] valid batch inserts rows
- [ ] duplicate delivery does not duplicate rows
- [ ] malformed batch goes to DLQ (no partial write)

### Task 6: Integration tests (the real gate)
Implement these tests (can run against docker compose):

1. Duplicate wakeups
2. Stale attempt fencing
3. Crash dispatcher mid-flight (restart, outbox resumes)
4. Crash worker mid-task (lease expires, retry completes)
5. Poison batch → DLQ

**Pass criteria**
- No double-commit of buffered rows
- Stale attempts cannot mutate state or publish
- Outbox side effects survive process restarts
