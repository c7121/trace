# Sink Extraction (Milestone 9)

Risk: Medium
Public surface: None

Summary: Move the buffer sink consumer (decode/validate/write + DLQ) from `harness/` into a reusable internal crate.

Plan:
- Add `crates/trace-sink` with the sink loop and message handler wired via `trace-core` `Queue`/`ObjectStore`.
- Keep `harness/src/sink.rs` as a thin wrapper that constructs lite adapters and calls into `trace-sink`.

Acceptance:
- `cd harness && cargo test -- --nocapture` stays green with no semantic changes to DLQ/idempotency behavior.
- Bad batches remain fail-closed: no partial DB writes and poison messages land in DLQ after retries.

Reduction:
- Reuse existing `trace-core` traits/adapters; add no new abstractions.
