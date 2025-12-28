# ADR 0005: Secrets Management

## Status
- Accepted

## Decision
- Use **AWS Secrets Manager** for application secrets (RPC keys, API keys, per-tenant secrets), accessed via IAM roles.
- **Worker wrapper** fetches secrets at task startup and injects them into operator environment.
- **Operator code** never calls Secrets Manager directly — receives secrets as env vars or config.

## Why
- Native AWS service with rotation/audit; Terraform-managed; fits least-privilege posture.
- Injection model keeps operator code simple and prevents accidental secret leakage.

## Consequences
- Provision secrets and access policies via Terraform.
- Worker wrapper IAM role has Secrets Manager read access; operator task role does not.
- Enforce logging/auditing of access; no long-lived keys on hosts.
- Optionally add SSM Parameter Store for non-sensitive config values.

## Open Questions
- None currently.
