# ADR 0002: Networking Posture

## Status
- Accepted

## Decision
- Enforce **no internet egress by default** for job containers.
- Permit outbound traffic only to:
  - **AWS VPC endpoints / PrivateLink** for required AWS APIs (e.g., S3, SQS, ECR, CloudWatch Logs, Secrets Manager).
  - **In-VPC services** (Dispatcher, sinks, query service) via private DNS / security groups.
  - **Designated egress services** (see below) that enforce an allowlist for external destinations.
- External egress (internet) is allowed only from dedicated, platform-managed egress services:
  - **Delivery Service / Webhook Egress Gateway** for outbound webhooks.
  - **RPC Egress Gateway** (or in-VPC nodes) for blockchain RPC access.
- Platform operators (e.g., `cryo_ingest`, `block_follower`) may access allowlisted RPC endpoints **only via the RPC egress gateway** (or an in-VPC node).

## Why
- Aligns with zero-trust posture (least egress, SOC2) and reduces exposure.
- User-defined jobs cannot exfiltrate data to arbitrary endpoints.

## Consequences
- Provision VPC endpoints for required AWS services (at minimum: S3 and SQS; typically also ECR, CloudWatch Logs, and Secrets Manager).
- Deploy RDS into **private subnets** (no public accessibility) and restrict access via security groups (not a VPC endpoint).
- Maintain allowlist of RPC provider endpoints per environment.
- Maintain allowlist of webhook URLs for alert delivery (per org or global).
- Default deny internet egress at the job/container security group. Explicitly allow only:
  - VPC endpoints,
  - in-VPC services,
  - and egress gateway services.
- Monitor and audit outbound traffic; alert on unexpected egress attempts.

## Open Questions
- Which enforcement mechanism will be used for destination allowlisting (HTTP CONNECT proxy, AWS Network Firewall with domain rules, PrivateLink where available)?
