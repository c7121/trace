# trace-core error contract (no anyhow in public API)

Risk: Medium
Public surface: `trace_core::Error`, `trace_core::Result<T>`; replace `anyhow::Result` in `Queue`, `ObjectStore`, `Signer`, and `lite::*` helpers.

Summary: Replace `anyhow::Result` in `trace-core` public APIs with a crate-owned error wrapper, keeping `anyhow` only as an internal implementation detail.

Plan:
- Introduce `trace_core::{Error, Result}` and convert public signatures.
- Update `harness` and `trace-query-service` call sites; preserve context strings.

Acceptance:
- `trace-core` rustdoc no longer shows `anyhow::Result` in public items.
- `?` propagation into `anyhow::Result` callers works via `From<trace_core::Error>`.

Reduction:
- Implement `Error` as a newtype over `anyhow::Error` (no new deps).
