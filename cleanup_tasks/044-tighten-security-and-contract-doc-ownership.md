# Cleanup Task 044: Tighten security and contract doc ownership

## Goal

Make security and wire-level contract docs easy to navigate with a single, unambiguous source of truth per surface (user routes, task auth, worker auth, credential minting, Lambda invocation).

## Why

The current docs are close, but they have a few high-risk sources of reader confusion:
- `docs/architecture/security.md` and `docs/architecture/invariants.md` both claim to define enforceable invariants.
- `docs/architecture/security.md` defines the "task principal" as untrusted compute only, but task-scoped endpoints are also used by trusted operator runtimes.
- `docs/architecture/user_api_contracts.md` can be misread as multi-org selection, while v1 deployment is documented as single-org.

## Assessment summary (from review task 024)

### Surface ownership map

| Surface | Owner doc | Dependent docs |
| --- | --- | --- |
| System invariants (global, enforceable) | `docs/architecture/invariants.md` | Most docs in `docs/architecture/` |
| Security model: threat model, trust boundaries, authn primitives | `docs/architecture/security.md` | `docs/architecture/invariants.md`, container docs |
| User API allowlist: user-reachable `/v1/*` routes | `docs/architecture/user_api_contracts.md` | Feature specs under `docs/specs/`, `docs/architecture/containers/gateway.md` |
| Internal wire contract index | `docs/architecture/contracts.md` | Contract docs under `docs/architecture/contracts/` |
| Task capability token format and verifier rules | `docs/architecture/contracts/task_capability_tokens.md` | `task_scoped_endpoints.md`, `credential_minting.md`, Query Service docs |
| Task-scoped Dispatcher endpoints (`/v1/task/*`) | `docs/architecture/contracts/task_scoped_endpoints.md` | `task_lifecycle.md`, `buffered_datasets.md`, `lambda_invocation.md` |
| Worker-only endpoints (`/internal/*`) | `docs/architecture/contracts/worker_only_endpoints.md` | Dispatcher container doc, ECS operator docs |
| Credential minting (`/v1/task/credentials`) | `docs/architecture/contracts/credential_minting.md` | `task_capability_tokens.md`, security model |
| Dispatcher to Lambda invocation payload | `docs/architecture/contracts/lambda_invocation.md` | UDF specs and lifecycle docs |
| Buffered dataset sink contract | `docs/architecture/contracts/buffered_datasets.md` | Alerting spec, ADR 0006 |

### Conflicts and drift risks

- **"Canonical invariants" overlap:** `docs/architecture/security.md` and `docs/architecture/invariants.md` both assert they define enforceable invariants. This should be resolved so readers know which file wins in a conflict.
- **Task principal definition too narrow:** `docs/architecture/security.md` frames the task principal as untrusted compute, but `docs/architecture/contracts/task_scoped_endpoints.md` explicitly includes trusted operator runtimes as callers.
- **Org selection ambiguity:** `docs/architecture/security.md` documents v1 as single-org, but `docs/architecture/user_api_contracts.md` does not explicitly say how `org_id` is derived in v1, which can be misread as "org comes from the JWT".

## Plan

- In `docs/architecture/security.md`:
  - Adjust the intro wording to avoid claiming ownership of global platform invariants owned by `docs/architecture/invariants.md`.
  - Refine the principal model to explicitly distinguish:
    - untrusted UDF task execution, and
    - trusted platform operator task execution,
    while keeping the capability-token and lease fencing story consistent for both.
  - Add a short "doc map" section that points to:
    - `docs/architecture/invariants.md` (global invariants),
    - `docs/architecture/contracts.md` (wire-level contracts),
    - `docs/architecture/user_api_contracts.md` (user route allowlist).
- In `docs/architecture/user_api_contracts.md`:
  - Clarify the v1 single-org assumption and explicitly state where `org_id` comes from in v1.
  - Make the remaining plain spec path a real Markdown link for consistency.
- In `docs/architecture/contracts.md`:
  - Keep it as an index, but make the "who owns what" story more explicit (brief ownership mapping and link-first guidance).
- In `docs/architecture/invariants.md`:
  - Keep security-relevant invariants, but reduce duplicated narrative by pointing readers to `docs/architecture/security.md` for the full security model.

## Files to touch

- `docs/architecture/security.md`
- `docs/architecture/user_api_contracts.md`
- `docs/architecture/contracts.md`
- `docs/architecture/invariants.md`

## Acceptance criteria

- There is one clearly stated owner for global invariants, and no doc claims authority it does not have.
- The security model accurately reflects that task-scoped endpoints may be used by trusted and untrusted task runtimes.
- `docs/architecture/user_api_contracts.md` makes the v1 org derivation explicit and is internally consistent with `docs/architecture/security.md`.
- Navigation is link-first and does not duplicate endpoint schemas across multiple files.

## Suggested commit message

`docs: clarify security and contracts ownership`

