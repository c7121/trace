# Runtime Invoker Interface (Milestone 10)

Risk: Medium
Public surface: None

Summary: Define a single `RuntimeInvoker` interface for “invoke untrusted UDF” with Lite and AWS (feature-gated) implementations.

Plan:
- Add `RuntimeInvoker` trait to `trace-core` using `UdfInvocationPayload` as the canonical request type.
- Implement Lite invoker in harness by routing existing `FakeRunner` through the trait; no behavior changes.
- Add AWS `AwsLambdaInvoker` in `trace-core` behind `aws` feature; compile-only at this milestone.

Acceptance:
- `cd harness && cargo test -- --nocapture` stays green (no semantic changes).
- `cd crates/trace-core && cargo check --features aws` succeeds.

Reduction:
- No new contracts/claims; only refactor to an interface.
