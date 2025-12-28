# Build Plan

**Version:** 1.0  
**Date:** December 2025

Phased approach: prove orchestration and data flow before user-facing features.

| Phase | Components | Validates |
|-------|------------|-----------|
| 0 | Terraform scaffolding (VPC, ECS, SQS, RDS, S3) | Infrastructure provisioning |
| 1 | Dispatcher + Lambda sources + Worker wrapper | Orchestration layer |
| 2 | `block_follower` → Postgres | Real-time ingestion to hot storage |
| 2 | `cryo_ingest` → S3 (parallel) | Historical backfill to cold storage |
| 3 | Query service + `query` job (hot only) | Query path works |
| 4 | `parquet_compact` | Hot → cold compaction lifecycle |
| 5 | Query service + `query` job (federated) | Query spans hot + cold |
| 6 | `alert_evaluate` + `alert_deliver` | User-facing alerting |
| 7 | `integrity_check` | Cold storage verification |

## Exit Criteria (concise)

For each phase, all bullets must be true before promotion.

- Functional: happy-path flow for the phase’s components demonstrated end-to-end.
- Reliability: retries/DLQ configured; >99% success in test env for scoped flows.
- Performance: basic throughput/latency target set and met for this phase’s scope.
- Operations: runbook + alerts for the new components are in place and exercised.
- Security: secrets/roles scoped; images/configs come from signed/artifacted sources.

### Deferred

See [backlog.md](backlog.md) for non-phase-specific deferred items.
