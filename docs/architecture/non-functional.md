# Non-Functional Requirements (Draft)

## Timeliness
- Near real-time data availability for alerting use cases.
- Alerts must not be missed or significantly delayed.

## Data Integrity
- No silent data loss.
- System detects and recovers from gaps or corruption.
- Outputs are verifiable (checksums, manifests).

## Reliability
- System recovers gracefully from failures; no unrecoverable states.
- Failed jobs are retried or dead-lettered for investigation.
- Audit trail for all operations and access.

## Scalability
- System can scale horizontally to handle increased load.
- Adding new chains, datasets, or users should not require redesign.
- Concurrent jobs (ingestion, backfills, queries) should not block each other.
- Tenant isolation: orgs are isolated; one org's load does not impact another.

## Cost Control
- System operates within budget constraints.
- Alerts on unexpected cost growth (RPC, storage, compute).
- Rate limits and quotas prevent runaway spend.

## Security
- Least privilege access throughout.
- Secrets managed securely; never in code or logs.
- Encryption in transit and at rest.
- **No internet egress by default**; allowlisted endpoints only (S3, RDS via VPC endpoints; pre-approved webhooks).
- PII access logged and auditable.

## Operations
- Infrastructure managed via IaC only.
- Observability: metrics, logs, traces for debugging and monitoring.
- Runbooks for common failure scenarios.
