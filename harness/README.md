# Trace contract-freeze harness (Rust)

This directory contains the **contract-freeze integration harness** for Trace.
It is intentionally minimal: prove leasing, fencing, outbox, and pointer-buffer semantics under at-least-once delivery
**before** building full feature implementations.

The harness runs in **Trace Lite** mode:

- Postgres **state** (control-plane) + pgqueue
- Postgres **data** (sink tables)
- MinIO (S3-compatible object store) for batch artifacts

## Quick start

Requirements:
- Rust stable
- Docker + docker compose

Bring up dependencies:

```bash
cd harness
docker compose up -d
```

Run migrations:

```bash
cd harness
cargo run -- migrate
```

Run the dispatcher / worker / sink in separate terminals:

```bash
cargo run -- dispatcher
cargo run -- worker
cargo run -- sink
```

Run integration tests (after the above is working):

```bash
cargo test
```

## Scope (v1 harness)

The harness only needs to exercise these flows:

- **Claim** a task lease (`/internal/task-claim`)
- **Heartbeat** a lease (`/v1/task/heartbeat`)
- **Publish** a buffer pointer (`/v1/task/buffer-publish`) — pointer pattern only
- **Complete** a task (`/v1/task/complete`)
- **Drain** outbox → queue (at-least-once)
- **Consume** buffer queue → validate → idempotent insert (DLQ on poison)

Anything else is intentionally out of scope.
