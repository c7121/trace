# Job Security Model

User-defined jobs (alerts, enrichments, custom transforms) can execute arbitrary code. The platform treats all user code as untrusted and enforces isolation at multiple layers.

## Threat Model

- **Malicious code**: data exfiltration, crypto mining, lateral attacks.
- **Buggy code**: infinite loops, memory leaks, crashes.
- **Resource abuse**: CPU/memory exhaustion, cost runaway.
- **Data access violations**: reading other orgs' data, unauthorized PII access.

## Container Isolation

- Each job runs in its own Fargate task (no shared compute with other jobs or orgs).
- **No privileged mode**: containers cannot access host resources.
- **Read-only root filesystem**: writes only to designated output paths.
- **No IAM role assumption**: task role has minimal, scoped permissions.
- **Secrets injection**: Worker wrapper fetches secrets from Secrets Manager and injects them into operator environment; operator code never calls Secrets Manager directly.

## Network Isolation

- Jobs run in a VPC with **no internet egress by default**.
- Allowlisted endpoints only:
  - S3 (via VPC endpoint)
  - RDS Postgres (via VPC endpoint)
  - SES/SNS (via VPC endpoint, for alert delivery)
  - Pre-approved webhook URLs (for alert delivery)
- User jobs cannot make arbitrary outbound HTTP calls.
- Platform jobs (e.g., `block_follower`, `cryo_ingest`) may access allowlisted RPC endpoints.
- No inbound connections to job containers.

## Resource Limits

- **CPU/memory**: hard caps in ECS task definition; job cannot exceed.
- **Execution timeout**: Worker terminates jobs exceeding max duration.
- **Disk quota**: ephemeral storage capped per task.
- **Rate limits**: max concurrent jobs and jobs-per-hour per org.
- **Cost alerts**: automated alerts when org approaches spend thresholds.

## Data Access Control

- **Scoped credentials**: each job receives credentials for only the datasets it's configured to read.
- **Org isolation**: queries are automatically filtered by `org_id`; jobs cannot access other orgs' data.
- **RPC access**:
  - **Platform jobs** (e.g., `block_follower`, `cryo_ingest`): may access allowlisted RPC endpoints.
  - **User jobs** (alerts, enrichments, custom transforms): query platform storage only, no raw RPC access.
- **PII gating**: jobs must be explicitly granted access to PII datasets; access is logged.

## Credential Handling

- Job receives short-lived, scoped tokens at execution time.
- Tokens grant:
  - Read access to declared input datasets.
  - Write access to declared output locations.
  - Invoke access to pre-approved webhook URLs.
- Tokens do not grant:
  - Access to other datasets.
  - IAM role assumption.
  - Secrets Manager access (secrets injected by Worker, not fetched by job).

## Audit and Monitoring

- All job executions logged: who, what, when, resource usage.
- All data access logged: datasets read, rows accessed.
- Anomaly detection: unusual resource consumption, access patterns.
- Abuse response: automatic job termination, org notification, potential suspension.

## Security Operations (best-practice defaults)

- **Signing & provenance**: Container images and DAG bundles are signed (e.g., cosign) with SBOMs published; deployments verify signatures before pull/apply. (Refs: [SLSA](https://slsa.dev), [CNCF Supply Chain Best Practices](https://github.com/cncf/tag-security/blob/main/community/working-groups/supply-chain-security/supply-chain-security-paper/sscsp.md), [PDF](https://raw.githubusercontent.com/cncf/tag-security/main/community/working-groups/supply-chain-security/supply-chain-security-paper/CNCF_SSCP_v1.pdf))
- **Secrets & rotation**: Short-lived, scoped credentials per job/org; stored secrets rotate on a fixed cadence (e.g., ≤90d) and on key events; no Secrets Manager access from user code. (Refs: [NIST SP 800-57](https://csrc.nist.gov/publications/detail/sp/800-57-part-1/rev-5/final), [NIST SP 800-63](https://pages.nist.gov/800-63-3/))
- **Egress allowlist workflow**: Changes require review/approval; allowlist is IaC-managed; no ad-hoc outbound endpoints. (Refs: [AWS egress restriction guidance](https://docs.aws.amazon.com/whitepapers/latest/aws-vpc-connectivity-options/egress-only.html), [AWS SCPs](https://docs.aws.amazon.com/organizations/latest/userguide/orgs_manage_policies_scps.html))
- **PII handling**: Datasets classified; PII access requires explicit grant + logging; jobs touching PII must be tagged and are subject to heightened audit/retention. (Refs: [GDPR Art. 5(1)(c) data minimization](https://gdpr.eu/article-5-how-to-process-personal-data/), [ISO 27001 Annex A.8](https://www.iso.org/standard/27001))
