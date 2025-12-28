# Product Requirements (Draft)

## Purpose
A general-purpose data platform for blockchain research and operations: safe, reliable, and extensible.

## Users
- Analysts and researchers
- DeFi teams
- Security professionals

## User stories
As an analyst or researcher, I can:
- **Curate** on-chain data — select, filter, and organize datasets from blockchain networks.
- **Combine** on-chain data with off-chain feeds — enrich blockchain data with external sources.
- **Enrich** data — add labels, annotations, and computed fields (both real-time and retroactive).
- **Alert** on data — define conditions and receive notifications on historical and live data.
- **Analyze** data — run summaries, aggregations, and models across the dataset.
- **Access both historical and real-time data** — seamless queries across full history and chain tip.

## Goals
- **Safe**: least privilege access; secrets managed securely; full audit trail.
- **Reliable**: no silent data loss; system recovers gracefully from failures.
- **Extensible**: variety of data in (on-chain, off-chain, batch, stream, push, pull); variety of operations out (query, enrich, alert, model).

## Non-goals
- Ultra-low-latency trading use cases
- On-prem deployment

## Dependencies/assumptions
- Cloud: AWS (initial target; design should not preclude portability).
- Chain: start with Monad (EVM-compatible); architecture supports adding chains later.
- IaC: only path to provision infrastructure; no manual changes.

## Open questions
- Budget guardrails for RPC and storage?

---

## Non-Functional Requirements

### Timeliness
- Near real-time data availability for alerting use cases.
- Alerts must not be missed or significantly delayed.

### Data Integrity
- No silent data loss.
- System detects and recovers from gaps or corruption.
- Outputs are verifiable (checksums, manifests).

### Reliability
- System recovers gracefully from failures; no unrecoverable states.
- Failed jobs are retried or dead-lettered for investigation.
- Audit trail for all operations and access.

### Scalability
- System can scale horizontally to handle increased load.
- Adding new chains, datasets, or users should not require redesign.
- Concurrent jobs (ingestion, backfills, queries) should not block each other.
- Tenant isolation: orgs are isolated; one org's load does not impact another.

### Cost Control
- System operates within budget constraints.
- Alerts on unexpected cost growth (RPC, storage, compute).
- Rate limits and quotas prevent runaway spend.

### Security
- Least privilege access throughout.
- Secrets managed securely; never in code or logs.
- Encryption in transit and at rest.
- **No internet egress by default**; allowlisted endpoints only (S3, RDS via VPC endpoints; pre-approved webhooks).
- PII access logged and auditable.

### Operations
- Infrastructure managed via IaC only.
- Observability: metrics, logs, traces for debugging and monitoring.
- Runbooks for common failure scenarios.
