# Security

Status: Draft
Owner: Platform
Last updated: 2026-01-02

This document is the canonical security artifact for Trace v1. It defines the **trust boundaries** and **enforceable invariants** the implementation MUST uphold.

## Scope and non-goals

**In scope (v1):**
- AWS deployment profile (API Gateway, private services, RDS, S3, SQS)
- Untrusted user code execution via `runtime: lambda` UDF runner
- Single-tenant, single-org deployment (schemas include `org_id` for future multi-org expansion)

**Not in scope (v1):**
- On-prem / BYO network perimeter assumptions
- Untrusted ECS UDF execution (deferred to v2; requires a different credential isolation story)

## Threat model

Primary threats Trace must be correct against:

- **Untrusted code execution**: user bundles attempt to exfiltrate data, escalate privileges, or tamper with task state.
- **Confused deputy**: a component with broad access is tricked into acting on behalf of another org/user/task.
- **Replay / stale attempt**: retries or duplicate deliveries attempt to commit outputs for a non-current attempt.
- **Ingress bypass**: internal callers exist (within VPC); no security boundary may rely only on API Gateway.
- **Provider compromise**: RPC/data providers can return stale/incorrect data; correctness checks must be possible.

## Trust boundaries

Trace uses three distinct identity contexts. Treat them as different “principals”:

1. **User principal** (end-user requests)
2. **Task principal** (untrusted compute: Lambda UDF runner)
3. **Worker principal** (trusted platform pollers/launchers)

The implementation MUST prevent principals from impersonating one another.

## Authentication and authorization

### User API calls (Bearer JWT)

User-facing endpoints (`/v1/...`) are authenticated with an OIDC JWT from the IdP.

- API Gateway SHOULD validate JWTs for edge rejection (WAF, rate limiting).
- Backend services MUST validate JWT signature + claims themselves and derive identity from the verified token.
- Backends MUST NOT treat forwarded headers (`X-Org-Id`, `X-User-Id`, etc.) as authoritative identity.

#### User JWT claim contract (v1)

Trace treats the OIDC JWT as **authentication** only. Authorization is derived from Postgres state.

Required JWT claims:
- `iss`, `aud`, `sub`, `exp`

Recommended (non-authoritative) claims:
- `email`

Authorization rules:
- `sub` maps to `users.external_id`.
- **v1 tenancy:** Trace v1 deploys as a single-org instance. Requests do not select an org.
- The backend resolves the single `org_id` from deployment configuration (or the single row in `orgs`).
- Effective role/permissions are resolved from `org_role_memberships` + `org_roles` for that `org_id`.
- If the user is not a member of the deployment org, the request MUST be rejected.

### Task-scoped APIs (capability tokens)

Task-scoped endpoints are callable by **untrusted** execution (the Lambda UDF runner). They are authenticated with a per-attempt **task capability token** (JWT) plus strict attempt fencing.

Invariants:
- The capability token is minted by Dispatcher per `(task_id, attempt)` and is time-limited (TTL must cover the task timeout, but it should not be long-lived).
- Requests MUST include `{task_id, attempt, lease_token}` and MUST be rejected if the lease token does not match the current attempt.
- Capability tokens grant only **task-scoped** rights: fenced task calls (heartbeat/complete/events/buffer publish), Query Service reads, and scoped credential minting. They do not grant direct Postgres access or broad AWS permissions.

### Worker-only APIs (worker tokens)

Some endpoints exist only because trusted platform workers need to claim and fetch tasks (e.g., for ECS platform operators).

- These endpoints MUST NOT be callable by untrusted code.
- Protect them by **network policy** (private subnets + security groups) AND a worker identity mechanism.
- v1 worker identity is a shared **worker token** (rotatable secret) injected only into the trusted worker wrapper/poller.

> mTLS is optional hardening for trusted service-to-service calls. It is NOT a primary mechanism for untrusted task auth.

## Secrets and key management

- **Untrusted task execution MUST NOT receive platform secrets** (no Secrets Manager access, no long-lived credentials).
- Platform-managed operators MAY receive secrets via:
  - ECS secret injection (Secrets Manager → env var in trusted container), or
  - other platform-controlled injection mechanisms.

JWT signing keys:
- AWS profile SHOULD use an asymmetric key managed by AWS KMS (ES256 recommended).
- Verifiers fetch public keys via an internal JWKS endpoint and cache them; rotation uses `kid`.

## Network and egress control

- All services run in private subnets; inbound paths are explicitly routed (API Gateway → internal ALB or VPC Link as applicable).
- Untrusted compute MUST have no third-party internet egress by default.
- External egress is mediated by:
  - RPC Egress Gateway (chain RPC)
  - Delivery Service (notifications/webhooks)

## Data access control

- Data is encrypted in transit (TLS) and at rest (RDS encryption, S3 SSE).
- Postgres state contains orchestration truth; Postgres data contains datasets and derived records.
- Untrusted tasks MUST NOT have direct Postgres credentials.
- Query Service is the primary read path for tasks and enforces capability-token-scoped reads.

## Audit and monitoring

Minimum audit requirements:
- Every user API request is attributable to `sub` and org.
- Every task mutation is attributable to `(task_id, attempt)` and runtime identity (task principal).
- Security-relevant events (failed auth, invalid lease_token, repeated completion attempts) are logged and alertable.

## Incident response defaults

- Token/key rotation paths must exist (JWKS `kid`, worker token rotation).
- Compromise response should include: revoke/rotate keys, invalidate outstanding capability tokens, and disable affected bundle IDs.

## Related

- `docs/standards/security_hardening.md` — implementation checklist (non-normative)
