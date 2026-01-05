# Dispatcher Extraction (Milestone 8)

Risk: Medium
Public surface: None

Summary: Move the dispatcher HTTP/router + background loops from `harness/` into a reusable internal crate.

Plan:
- Add `crates/trace-dispatcher` containing the dispatcher router, handlers, outbox drainer, and lease reaper.
- Keep `harness/src/dispatcher.rs` as a thin wrapper that wires config + lite adapters and exposes the existing `DispatcherServer` API.

Acceptance:
- `cd harness && cargo test -- --nocapture` stays green with no endpoint/DB semantic changes.
- Harness still controls enable/disable of outbox and lease reaper loops.

Reduction:
- Reuse existing `trace-core` traits/adapters; add no new abstractions.
