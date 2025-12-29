# TODO

Canonical task list for Trace development. See [build.md](docs/plan/build.md) for phase summary.

## Format

```
- [ ] **Task name** — Brief description. See [link](#) for spec.
```

Move to "In Progress" when starting, "Done" when complete.

---

## Next Up

### Spec Gaps
- [ ] **UDF spec** — Define user-defined function model for alerts. Function signature, storage format, available libraries, sandbox constraints, timeout/limits. Blocks alert_evaluate. See [udf.md](docs/capabilities/udf.md).

### Phase 0: Infrastructure
- [ ] **Bootstrap tfstate** — Create S3 bucket + DynamoDB table for Terraform state.
- [ ] **Terraform: VPC** — VPC, public/private subnets, route tables, NAT. See [ADR 0002](docs/architecture/adr/0002-networking.md).
- [ ] **Terraform: VPC endpoints** — S3, RDS, SES, SNS, Secrets Manager. Zero egress by default.
- [ ] **Terraform: RDS Postgres** — Single instance, private subnet, security group.
- [ ] **Terraform: S3 buckets** — Cold storage bucket, lifecycle policies.
- [ ] **Terraform: SQS** — FIFO queue + DLQ per runtime.
- [ ] **Terraform: ECS cluster** — Fargate cluster, task execution role.
- [ ] **Terraform: ECR repos** — One repo per operator image.
- [ ] **Terraform: Secrets Manager** — Initial structure, IAM policies.
- [ ] **Terraform: CloudWatch** — Log groups, baseline metrics/alarms.
- [ ] **Smoke test** — Network reachability, DB connectivity, SQS send/receive.

**Acceptance**: `terraform apply` succeeds; ECS tasks in private subnets reach S3/SQS/RDS via endpoints; outbound to non-allowlisted internet blocked; CloudWatch logs ingest a test entry.

### Phase 1: Orchestration
- [ ] **Postgres schema** — Core tables: `orgs`, `users`, `org_roles`, `org_role_memberships`, `jobs`, `tasks`, `task_inputs`. See [orchestration.md](docs/capabilities/orchestration.md).
- [ ] **Postgres schema** — Versioning tables: `partition_versions`, `dataset_cursors`, `data_invalidations`. See [data_versioning.md](docs/architecture/data_versioning.md).
- [ ] **Dispatcher service** — Create tasks, enqueue to SQS, reaper, source monitor, upstream event routing. See [readme.md](docs/readme.md#1-dispatcher).
- [ ] **DAG sync** — Parse YAML, validate, upsert jobs to Postgres. See [dag_deployment.md](docs/architecture/dag_deployment.md).
- [ ] **Lambda sources** — Cron source (EventBridge), webhook source (API Gateway), manual source.
- [ ] **Worker wrapper** — Fetch task from Dispatcher, fetch/inject secrets, heartbeat, execute operator, report status. See [contracts.md](docs/architecture/contracts.md).
- [ ] **Gateway stub** — Basic API for manual triggers (full Gateway deferred).

**Acceptance**: Lambda emits event → Dispatcher creates tasks → worker executes noop operator; task status transitions Queued→Running→Completed recorded; heartbeat + reaper kill a stalled task; DAG sync creates jobs from YAML.

### Phase 2: Ingestion
- [ ] **Hot storage schema** — `hot_blocks`, `hot_transactions`, `hot_logs` tables in Postgres.
- [ ] **block_follower operator** — Follow chain tip, write to Postgres, handle reorgs, emit invalidations. See [block_follower.md](docs/architecture/operators/block_follower.md).
- [ ] **cryo_ingest operator** — Backfill historical data to S3 Parquet. See [cryo_ingest.md](docs/architecture/operators/cryo_ingest.md).
- [ ] **Upstream events** — block_follower emits events per block/batch.
- [ ] **Validate tip freshness** — Confirm block_follower keeps up with chain tip.

**Acceptance**: block_follower ingests N consecutive blocks with no gaps; reorg simulation rolls back hot rows and creates `data_invalidations` entry; upstream events fire; cryo_ingest writes Parquet to S3.

### Phase 3: Query (Hot)
- [ ] **duckdb_query operator (hot only)** — Query Postgres via DuckDB. See [duckdb_query.md](docs/architecture/operators/duckdb_query.md).
- [ ] **Query API** — Basic endpoint for SQL queries.
- [ ] **Authn/authz enforcement** — org_id filtering, role checks.

**Acceptance**: duckdb_query returns correct results for a known dataset; org/role filtering enforced.

### Phase 4: Compaction
- [ ] **parquet_compact operator** — Compact hot → cold past finality threshold. See [parquet_compact.md](docs/architecture/operators/parquet_compact.md).
- [ ] **Finality threshold config** — Per-chain finality settings.
- [ ] **Update partition_versions** — Record compacted partitions.
- [ ] **Optional hot cleanup** — Delete compacted data from Postgres.

**Acceptance**: parquet_compact compacts finalized ranges to S3; partition_versions updated; optional hot delete works.

### Phase 5: Query (Federated)
- [ ] **duckdb_query federated** — Query across Postgres + S3.
- [ ] **Validate correctness** — Results match across hot/cold boundaries.

**Acceptance**: Federated query returns same totals as separate hot+cold queries on a test dataset.

### Phase 6: Alerting
- [ ] **Alert schema** — `alert_definitions`, `alert_events` tables. See [alerting.md](docs/capabilities/alerting.md).
- [ ] **Alert definitions API** — CRUD for alert rules.
- [ ] **alert_evaluate operator** — Pick one runtime first (TS/Py/Rust). See [alert_evaluate_*.md](docs/architecture/operators/).
- [ ] **alert_deliver operator** — Email/SMS/webhook delivery. See [alert_deliver.md](docs/architecture/operators/alert_deliver.md).
- [ ] **Channel rate limits** — Per-channel throttling.

**Acceptance**: alert_evaluate triggers on a test condition; alert_deliver sends webhook/email; channel rate-limit enforced.

### Phase 7: Integrity
- [ ] **integrity_check operator** — Verify cold storage against canonical chain. See [integrity_check.md](docs/architecture/operators/integrity_check.md).
- [ ] **Scheduled verification** — Periodic integrity job via cron source.

**Acceptance**: integrity_check detects a tampered cold file and reports it.

### Deferred

See [backlog.md](docs/plan/backlog.md) for non-phase-specific deferred items:
- User-defined jobs (arbitrary code execution)
- Multi-chain support beyond Monad
- Physical tenant isolation (per-org infra)
- UI/dashboard
- Enterprise Integration Patterns (Wire Tap, Aggregator, Correlation ID, Message History)

---

## In Progress

_None_

---

## Done

_None_
