# Harness assumptions (Trace Lite)

- `state.queue_messages.message_id` is a UUID (per `harness/migrations/state/0001_init.sql`), so the `PgQueue` adapter uses UUID message IDs.
- Task capability tokens are JWTs using HS256 with a local dev secret (`TASK_CAPABILITY_SECRET`) instead of an asymmetric keypair/JWKS (sufficient for harness invariants).
- MinIO bucket is configured for anonymous read/write in `harness/docker-compose.yml` so the harness can use simple HTTP PUT/GET without implementing AWS SigV4 signing.
- `TRACE_CRYO_MODE=real` shells out to `cryo` using the assumed CLI shape: `cryo <dataset> --rpc-url <url> --start-block <n> --end-block <n> --output-dir <dir>` (if Cryo CLI differs, update `harness/src/cryo_worker.rs` accordingly).
