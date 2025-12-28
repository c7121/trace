# ADR 0006: Networking Posture

## Status
- Accepted

## Decision
- Enforce **no internet egress by default** for job containers.
- Allow egress only to:
  - S3 (via VPC endpoint)
  - RDS Postgres (via VPC endpoint)
  - Pre-approved webhook URLs (for alert delivery)
- Platform operators (e.g., `cryo_ingest`, `block_follower`) may access allowlisted RPC endpoints.

## Why
- Aligns with zero-trust posture (least egress, SOC2) and reduces exposure.
- User-defined jobs cannot exfiltrate data to arbitrary endpoints.

## Consequences
- Provision VPC endpoints for S3 and RDS.
- Maintain allowlist of RPC provider endpoints per environment.
- Maintain allowlist of webhook URLs for alert delivery (per org or global).
- Default deny egress in security groups; explicit allow for allowlisted destinations.
- Monitor and audit outbound traffic; alert on unexpected egress attempts.

## Open Questions
- None currently.
