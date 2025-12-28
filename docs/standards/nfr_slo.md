# Non-Functional Requirements & SLOs

Measurable targets and global constraints for the platform.

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
- **Idempotency**: re-running a job for the same inputs produces the same outputs.
- **Failover**: detect issues and reroute to healthy alternatives.
- **Rate limits**: respect external provider limits; back off when throttled.

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

See [security.md](security.md) for detailed security requirements.

- Least privilege access throughout.
- Secrets managed securely; never in code or logs.
- Encryption in transit and at rest.
- **No internet egress by default**; allowlisted endpoints only.
- PII access logged and auditable.

## Operations

- Infrastructure managed via IaC only.
- Observability: metrics, logs, traces for debugging and monitoring.
- Runbooks for common failure scenarios.

## Observability

- **Metrics**: system emits job success/error rates, latency, throughput, cost signals
- **Logs/traces**: correlated across job runs; secrets redacted
- **System alerts**: system alerts on failures and anomalies (separate from user-defined alerts)

### Key Metrics by SLO

| SLO Area | Metric | Description |
|----------|--------|-------------|
| Timeliness | `ingest_lag_seconds` | Delay from chain tip to hot storage |
| Timeliness | `alert_delivery_lag_seconds` | Time from trigger to delivery |
| Data Integrity | `task_success_rate` | % tasks completed without error |
| Data Integrity | `dlq_depth` | Dead-letter queue size |
| Reliability | `dispatcher_queue_depth` | Pending tasks per queue |
| Reliability | `dispatcher_queue_age_p95` | Age of oldest pending task |
| Reliability | `task_retry_count` | Retries per task histogram |
| Scalability | `worker_utilization` | CPU/memory per worker pool |
| Scalability | `concurrent_tasks` | In-flight tasks by operator |
| Cost Control | `rpc_requests_total` | RPC calls by provider |
| Cost Control | `storage_bytes_written` | S3/Postgres writes |
| Query | `query_duration_p95` | Interactive query latency |
| Query | `query_queue_depth` | Pending interactive queries |
