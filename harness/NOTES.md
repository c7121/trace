# Harness assumptions (Trace Lite)

This file captures harness-specific deltas from the canonical docs.

Canonical owner docs:
- Interface contracts index: [docs/architecture/contracts.md](../docs/architecture/contracts.md)
  - Task capability tokens: [docs/architecture/contracts/task_capability_tokens.md](../docs/architecture/contracts/task_capability_tokens.md)
  - Task-scoped endpoints: [docs/architecture/contracts/task_scoped_endpoints.md](../docs/architecture/contracts/task_scoped_endpoints.md)
- Security model: [docs/architecture/security.md](../docs/architecture/security.md)
- Cryo ingest and range semantics: [docs/specs/operators/cryo_ingest.md](../docs/specs/operators/cryo_ingest.md)

- `state.queue_messages.message_id` is a UUID (per `migrations/state/0001_init.sql`), so the `PgQueue` adapter uses UUID message IDs.
- Task capability tokens are JWTs using HS256 with a local dev secret (`TASK_CAPABILITY_SECRET`) instead of an asymmetric keypair/JWKS (sufficient for harness invariants).
- MinIO bucket is configured for anonymous read/write in `docker-compose.yml` so the harness can use simple HTTP PUT/GET without implementing AWS SigV4 signing.
- `TRACE_CRYO_MODE=real` shells out to `cryo` using the CLI shape `cryo <dataset> --rpc <url> --blocks <start:end> --output-dir <dir>`.
  - Source of truth: `src/cryo_worker.rs`.
  - Range semantics are start-inclusive and end-exclusive: pass `--blocks {start}:{end}` without pre-decrement.
