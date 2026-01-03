# ADR 0002: Networking Posture

## Status
- Accepted

## Decision
- Enforce **no internet egress by default** for job containers **and VPC-attached Lambdas** (sources + UDF runners).
- Permit outbound traffic only to:
  - **AWS VPC endpoints / PrivateLink** for required AWS APIs (e.g., S3, SQS, ECR, CloudWatch Logs; Secrets Manager only for platform-managed tasks via ECS secret injection).
  - **In-VPC services** (Dispatcher, sinks, query service) via private DNS / security groups.
  - **Designated egress services** (see below) for any outbound internet access.
- External egress (internet) is allowed only from dedicated, platform-managed egress services:
  - **Delivery Service / Webhook Egress Gateway** for outbound webhooks.
  - **RPC Egress Gateway** (or in-VPC nodes) for blockchain RPC access.
- Platform operators (e.g., `cryo_ingest`, `block_follower`) may access RPC providers **only via the RPC Egress Gateway** (or an in-VPC node).

## Why
- Aligns with zero-trust posture (least egress, SOC2) and reduces exposure.
- User-defined compute cannot make arbitrary outbound requests; all external communication flows through platform egress services with auditing.

## Consequences
- Provision VPC endpoints for required AWS services (at minimum: S3 and SQS; typically also STS, KMS, ECR, CloudWatch Logs, and Secrets Manager for trusted services).
- Configure any Lambda that must call internal services (Dispatcher, Query Service, sinks) as **VPC-attached** in private subnets with **no NAT**. This preserves internal-only endpoints and prevents untrusted code from gaining arbitrary internet egress.
- Deploy RDS into **private subnets** (no public accessibility) and restrict access via security groups (not a VPC endpoint).
- Default deny internet egress at the job/container security group. Explicitly allow only:
  - VPC endpoints,
  - in-VPC services,
  - and egress gateway services.
- Monitor and audit outbound traffic; alert on unexpected egress attempts.

**Implementation note:** Enforcement is primarily via network topology: job containers have no default route/NAT, and only egress services can reach the public internet.
