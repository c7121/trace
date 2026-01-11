# Task Capability Tokens

Task-scoped endpoints are authenticated with a per-attempt **task capability token** plus strict attempt fencing.

This document defines the token format, claims, and verifier rules.

## Format (v1)

- The task capability token is a JWT signed by the Dispatcher.
  - AWS/prod: asymmetric signing (recommended: ES256) with a public JWKS for verifiers.
  - Lite/harness: HS256 is acceptable for local development only (shared secret via env); do not treat this as a security boundary.
- Verifiers (Dispatcher `/v1/task/*`, Query Service `/v1/task/query`, sinks) validate signature and expiry:
  - AWS/prod: using the Dispatcher internal task-JWKS document (public keys only).
  - Lite/harness: using the shared HS256 secret configured out of band.
- The task-JWKS endpoint is internal-only (for example `GET /internal/jwks/task`) and should be cached by verifiers; rotation uses `kid`.

## Claims (v1, normative)

Header:
- `kid` (required): key identifier for rotation.

Standard claims (required unless noted):
- `iss`: Dispatcher issuer identifier
- `aud`: `trace.task`
- `sub`: `task:{task_id}`
- `exp`: expiry (short-lived)
- `iat` (recommended)
- `jti` (optional; for future replay detection)

Custom claims (required):
- `org_id`: UUID (deployment org; v1 is single-org but the claim is still required)
- `task_id`: UUID
- `attempt`: int
- `datasets`: list of dataset grants for `/v1/task/query` (may be empty)
  - required: `{dataset_uuid, dataset_version}`
  - required for Parquet attach: `storage_ref` (fail-closed if missing when attach is required)
    - S3: `{scheme:"s3", bucket, prefix, glob}`
    - local dev: `{scheme:"file", prefix, glob}`
  - Query Service uses this to attach authorized datasets as DuckDB relations (trusted attach), then executes gated SQL (untrusted).
- `s3`: `{read_prefixes[], write_prefixes[]}` where prefixes are canonical `s3://bucket/prefix/` strings for `/v1/task/credentials` (may be empty in Lite)

## Verifier rules (v1)

- MUST validate JWT signature and required claims (`kid`, `iss`, `aud`, `exp`).
- MUST bind the request body `{task_id, attempt}` to the token claims.
- Dispatcher task endpoints MUST additionally enforce `{lease_token}` fencing against the current lease.
- Deny-by-default: if a required claim is missing or malformed, reject.

Token size note: keep capability tokens small. If scopes become large (many dataset grants/prefixes), switch to a `capability_id` claim and resolve the full capability from Postgres state.

## UDF data access token usage

For untrusted UDF tasks, the Dispatcher issues a short-lived capability token (passed to the runtime as an env var such as `TRACE_TASK_CAPABILITY_TOKEN`).

The token is the single source of truth for what the UDF is allowed to read and write during the attempt:
- Allowed input datasets (pinned dataset versions) and their resolved storage locations
- Allowed output prefix (S3)
- Allowed scratch or export prefix (S3)

The token is enforced by:
- Query Service - for ad-hoc SQL reads across Postgres and object storage; only the datasets in the token are attached as views.
- Dispatcher - exchanges the token for short-lived STS credentials scoped to the allowed S3 prefixes (credential minting).

In ECS, a trusted worker wrapper typically performs task and lease calls and credential minting and then injects the resulting scoped credentials into the untrusted process.

In `runtime: lambda`, the Lambda uses the capability token directly (there is no wrapper boundary).

UDF code never connects to Postgres directly.

## Related

- Task-scoped endpoints: [task_scoped_endpoints.md](task_scoped_endpoints.md)
- Credential minting: [credential_minting.md](credential_minting.md)
- Security model: [security.md](../security.md)
