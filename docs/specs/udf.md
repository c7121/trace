# User-defined functions

Status: Draft
Owner: Platform
Last updated: 2026-01-02

## Summary
UDFs are user-supplied code bundles executed by the platform to implement alert conditions and (later) custom transforms/enrichments. UDFs are treated as **untrusted** and interact with the platform only through task-scoped APIs using capability tokens.

## Risk
High

UDFs change trust boundaries and introduce arbitrary code execution.

## Problem statement
Users want custom logic without the platform hardcoding every rule/transform. We need a design that:
- supports multiple languages,
- is safe under zero trust,
- is operable in AWS without per-task IAM role explosions.

Constraints:
- User code is untrusted in all runtimes, including `runtime: lambda`.
- Task execution is at-least-once; UDFs must be idempotent or emit deterministic idempotency keys.
- UDFs must not hold long-lived secrets or broad AWS permissions.

## Goals
- Execute untrusted UDF bundles with minimal privileges.
- Allow UDFs to read inputs via Query Service and write outputs via task-scoped publish APIs.
- Support a platform-managed Lambda runner for v1 (good isolation, low ops).
- Keep the security model identical across AWS and Trace Lite profiles.

## Non-goals
- Allowing UDFs to make arbitrary outbound network calls.
- Allowing UDFs to connect directly to Postgres.
- Supporting long-running UDF jobs that require hours of continuous execution (v1).

## Public surface changes
- Config semantics: `runtime: lambda` is allowed for UDF jobs and is executed via a platform-managed runner.
- Persistence format: UDF bundle format (see ADR) and signed provenance requirements.

## Architecture (C4) — Mermaid-in-Markdown only

```mermaid
flowchart LR
  DISP[Dispatcher] -->|invoke + task tokens| RUN[Lambda UDF runner]
  RUN -->|task query (capability token)| QS[Query Service]
  RUN -->|buffer publish (worker token)| DISP
```

## Proposed design

### Runtime model (v1)
- `runtime: lambda` executes UDF bundles inside a **platform-managed Lambda runner**.
  - The runner fetches the bundle via a **pre-signed URL** minted by Dispatcher.
  - The runner receives two task-scoped JWTs:
    - **Capability token** (data-plane): Query Service reads, credential minting.
    - **Worker token** (task-plane): heartbeat/complete/buffer publish.
  - The runner Lambda execution role is near-zero (logs + networking only). It should not have broad S3/SQS/Secrets permissions.
- `ecs_udf` is **deferred to v2** for untrusted code. ECS tasks share IAM creds across containers, so a secure design requires additional isolation work (see “ECS UDF (v2) hurdle”).


### Referencing bundles in DAG config

A DAG job that runs user code MUST include an `udf` block:

```yaml
- name: my_udf_job
  activation: reactive
  runtime: lambda
  operator: udf
  outputs: 1
  inputs:
    - from: { dataset: some_dataset }
  update_strategy: append
  unique_key: [dedupe_key]
  udf:
    bundle_id: "<bundle-id>"
    entrypoint: "trace.handler"
```

- `bundle_id` is the immutable identifier of a previously uploaded bundle.
- `entrypoint` is the handler function inside the bundle (per ADR 0003).

See `docs/specs/dag_configuration.md` for the job schema.

### Bundle format and provenance
- Bundle format and entrypoints are defined in `docs/adr/0003-udf-bundles.md`.
- UDF bundles MUST be signed/validated and associated with an org + user for auditability. See `docs/standards/security_model.md`.

### Data access
- UDFs read data via Query Service using the **capability token**. Query Service enforces dataset/version pinning and org scoping.
- If a UDF needs to read/write S3 artifacts directly, it requests scoped credentials via task-scoped APIs (short-lived, prefix-scoped).

### Output and idempotency
- UDF outputs must use declared semantics:
  - `replace`: write attempt-scoped artifacts, commit via Dispatcher
  - `append`: emit deterministic keys and rely on sink upsert semantics
- UDFs MUST NOT emit attempt-derived idempotency keys (those change on retry). Idempotency keys must be derived from domain data (or a stable upstream cursor key).

### ECS UDF (v2) hurdle (zero-trust requirement)
Before untrusted `ecs_udf` is supported, the design MUST prevent untrusted code from inheriting privileged AWS credentials (e.g., SQS poller creds) and must retain:
- task-scoped capability tokens,
- no direct Postgres access,
- bounded AWS permissions per execution.

## Contract requirements
- UDFs MUST be treated as untrusted in all runtimes.
- UDFs MUST authenticate only via task-scoped JWTs over TLS (no hidden shared secrets).
- UDFs MUST NOT have direct Postgres connectivity.
- The platform MUST provide clear failure reporting (stderr, structured error) without leaking secrets.

## Security considerations
- Threats: arbitrary code execution, token exfiltration, data exfiltration, denial of service.
- Mitigations:
  - near-zero IAM roles for UDF runner,
  - task-scoped short-lived JWTs + lease fencing,
  - no network egress to third parties,
  - resource limits and timeouts.
- Residual risk: user code can still burn CPU/memory; mitigated with quotas and timeouts.

## Alternatives considered
- mTLS client auth for task-scoped endpoints.
  - Why not: untrusted runtimes cannot safely hold client certs; operationally heavy.
- Give UDFs broad IAM permissions and rely on policy.
  - Why not: violates least privilege and makes abuse harder to contain.

## Acceptance criteria
- Tests:
  - UDF runner can execute a bundle and query data only permitted by its capability token.
  - UDF cannot access Secrets Manager or arbitrary S3 prefixes.
  - Duplicate UDF retries do not create duplicate outputs (idempotent append or replace commit).
- Observable behavior:
  - Per-run logs/metrics exist; token issuance/validation failures are visible.
