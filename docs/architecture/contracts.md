# Interface Contracts

Wire-level contracts for internal task execution in Trace v1: tokens, payloads, and endpoint rules.

Systemwide invariants that constrain these contracts live in [invariants.md](invariants.md).
The user-facing Gateway surface is enumerated in [user_api_contracts.md](user_api_contracts.md).

Scope:
- Task capability token format and verifier rules
- Task-scoped Dispatcher endpoints (`/v1/task/*`)
- Worker-only Dispatcher endpoints (`/internal/*`)
- Dispatcher to Lambda invocation payload (`runtime: lambda`)
- Buffered dataset publish and sink contract

## Invariants

`/internal/*` endpoints are internal-only and are not exposed to end users. They are callable only by trusted platform components (worker wrappers and platform services). Untrusted runtimes (UDF code, including `runtime: lambda`) must not call `/internal/*`.

Transport: TLS is required for all internal APIs.

Auth model:
- Task-scoped endpoints (heartbeat, completion, events, buffer publish) are authenticated with a short-lived task capability token plus `{task_id, attempt, lease_token}` fencing.
- Worker-only endpoints (task claim, task fetch) are callable only by trusted worker wrappers and are protected by network policy plus a worker identity mechanism.
- Privileged platform endpoints (if any) should use a separate service identity mechanism (recommended: service JWT); mTLS is optional hardening, not a requirement.

Delivery semantics: tasks and upstream events are at-least-once. Duplicates and out-of-order delivery are expected; correctness comes from attempt and lease gating plus idempotent output commits. See [task_lifecycle.md](task_lifecycle.md).

Endpoints under `/v1/task/*` are internal-only and must not be routed through the public Gateway.

## Contract index

- Task capability tokens: [contracts/task_capability_tokens.md](contracts/task_capability_tokens.md)
- Task-scoped endpoints: [contracts/task_scoped_endpoints.md](contracts/task_scoped_endpoints.md)
- Worker-only endpoints and queue wake-ups: [contracts/worker_only_endpoints.md](contracts/worker_only_endpoints.md)
- Credential minting: [contracts/credential_minting.md](contracts/credential_minting.md)
- Dispatcher to Lambda invocation: [contracts/lambda_invocation.md](contracts/lambda_invocation.md)
- Buffered dataset sink contract: [contracts/buffered_datasets.md](contracts/buffered_datasets.md)
