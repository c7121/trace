# Build Plan

**Version:** 1.1  
**Date:** December 2025

Phased approach: prove orchestration and data flow before user-facing features.

## Phase Summary

| Phase | Focus | Validates |
|-------|-------|-----------|
| 0 | Infrastructure | Terraform scaffolding (VPC, ECS, SQS, RDS, S3) |
| 1 | Orchestration | Dispatcher, Lambda sources, Worker wrapper, DAG sync |
| 2 | Ingestion | `block_follower` → Postgres, `cryo_ingest` → S3 |
| 3 | Query (Hot) | DuckDB queries against Postgres |
| 4 | Compaction | `parquet_compact` hot → cold |
| 5 | Query (Federated) | DuckDB spans Postgres + S3 |
| 6 | Alerting | `alert_evaluate` + `alert_deliver` |
| 7 | Integrity | `integrity_check` cold storage verification |

## Task Tracking

See [TODO.md](../../TODO.md) for the canonical task list with detailed acceptance criteria.

## Exit Criteria

For each phase, all bullets must be true before promotion:

- **Functional**: happy-path flow demonstrated end-to-end
- **Reliability**: retries/DLQ configured; >99% success in test env
- **Performance**: throughput/latency target set and met
- **Operations**: runbook + alerts in place and exercised
- **Security**: secrets/roles scoped; signed images/configs

## Deferred

See [backlog.md](backlog.md) for non-phase-specific deferred items.
