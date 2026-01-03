# Security hardening

Status: Draft
Owner: Platform
Last updated: 2026-01-02

This document captures **implementation hardening guidance** for Trace v1.

- **Normative security invariants** live in `docs/standards/security_model.md`.
- This document is intentionally a checklist: apply what is useful for your threat model without turning v1 into an operations science project.

## Secrets handling

### Baseline requirements
- **Untrusted runtimes** (UDF bundles, including `runtime: lambda`) MUST NOT receive platform secrets.
- Platform operators SHOULD receive secrets via **injection at launch** (ECS task definition `secrets` → env vars; or Lambda env vars), not by calling Secrets Manager at runtime.
- Do not log secrets or tokens. Treat JWTs (user or task) as secrets.

### Recommended naming convention (AWS Secrets Manager)
Keep it boring and grep-able. A path-like name works well:

- `/{env}/trace/{component}/{secret_name}`

Examples:
- `/prod/trace/postgres_state/password`
- `/prod/trace/postgres_data/password`
- `/prod/trace/dispatcher/task_jwt_signing_key_id` (if you store a KMS key id)
- `/prod/trace/workers/worker_token`

Notes:
- If/when multi-tenant is introduced, add a tenant segment (e.g., `/{env}/trace/{org_slug}/...`). Do not pre-bake org scoping into v1 operational workflows if you are deploying single-tenant.

### Rotation
Minimum rotation paths to implement up front:
- Worker token rotation (old+new overlap window; wrappers reload without redeploy).
- Task-JWKS key rotation via `kid` (verifiers cache, accept both keys for a window).
- Postgres credentials rotation (ideally via RDS managed rotation if you can accept the constraints).

## Encryption

Baseline:
- TLS for all token-bearing calls.
- Encrypt RDS volumes and snapshots.
- S3 server-side encryption enabled.

Recommended defaults:
- Prefer **SSE-KMS** for buckets that may contain PII-bearing datasets, scratch/query exports, or alert payloads.
- Enforce HTTPS-only bucket policies for all Trace buckets.

## Supply chain and artifact integrity

Keep v1 simple but do not skip the minimum integrity hooks:

- **UDF bundles** MUST be content-addressed:
  - record SHA-256 at upload time,
  - verify SHA-256 before execution.
- **Container images** SHOULD be pinned by digest in deployment manifests.

Optional (good later, not required for MVP):
- Image signing (cosign) + verification in CI/CD.
- SBOM generation and storage for platform images.

## Logging and audit

Baseline:
- Log enough to attribute:
  - user requests → `(iss, sub)` and resolved `(user_id, org_id)`
  - task mutations → `(task_id, attempt)`
- Never log raw bearer tokens.

Recommended:
- Separate “audit log” stream from “debug log” stream (different retention, different access).
- Retain audit logs long enough for incident response (pick an explicit number; 30–90 days is a common starting point).

## Network

Baseline:
- Public ingress is only via API Gateway.
- Internal ALB is private-only.
- Untrusted compute has no third-party egress by default.

Recommended:
- Add explicit egress allowlists (VPC endpoints, security group egress rules) rather than relying on “no NAT” as the only guardrail.
