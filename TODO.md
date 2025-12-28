# TODO

Task list for agentic handoff between Claude and Codex.

## Format

```
- [ ] **Task name** — Brief description. See [link](#) for spec.
```

Move to "In Progress" when starting, "Done" when complete.

---

## Next Up

### Phase 0: Infrastructure
- [ ] **Bootstrap tfstate** — Create S3 bucket + DynamoDB table for Terraform state. Manual or bootstrap script.
- [ ] **Terraform: VPC** — VPC, public/private subnets, route tables, NAT (if needed). See [ADR 0006](docs/architecture/adr/0006-networking.md).
- [ ] **Terraform: VPC endpoints** — S3, RDS, SES, SNS, Secrets Manager. Zero egress by default.
- [ ] **Terraform: RDS Postgres** — Single instance, private subnet, security group. See [architecture.md](docs/architecture/architecture.md#5-postgres).
- [ ] **Terraform: S3 buckets** — Cold storage bucket, lifecycle policies. See [architecture.md](docs/architecture/architecture.md#6-asset-storage).
- [ ] **Terraform: SQS** — FIFO queue + DLQ. See [architecture.md](docs/architecture/architecture.md#3-sqs-queue).
- [ ] **Terraform: ECS cluster** — Fargate cluster, task execution role. See [architecture.md](docs/architecture/architecture.md#4-workers).
- [ ] **Terraform: ECR repos** — One repo per operator image.
- [ ] **Terraform: Secrets Manager** — Initial structure, IAM policies. See [ADR 0005](docs/architecture/adr/0005-secrets.md).
- [ ] **Terraform: CloudWatch** — Log groups, baseline metrics/alarms. See [ADR 0004](docs/architecture/adr/0004-observability.md).
- [ ] **Smoke test** — Network reachability, DB connectivity, SQS send/receive.

**Acceptance**: `terraform apply` succeeds; ECS tasks in private subnets reach S3/SQS/RDS via endpoints; outbound to non-allowlisted internet blocked; CloudWatch logs ingest a test entry.

### Phase 1: Orchestration
- [ ] **Postgres schema** — jobs, tasks, triggers, data_partitions, orgs, users tables. See [architecture.md Data Model](docs/architecture/architecture.md#data-model).
- [ ] **Dispatcher service** — Create tasks, enqueue to SQS, reaper, singleton monitor. See [architecture.md](docs/architecture/architecture.md#2-dispatcher).
- [ ] **Trigger service** — Cron evaluator, webhook handler, threshold listener. See [architecture.md](docs/architecture/architecture.md#1-trigger-service).
- [ ] **Worker wrapper** — Fetch task, fetch/inject secrets, heartbeat, execute operator, ack. See [architecture.md](docs/architecture/architecture.md#4-workers).

**Acceptance**: Dispatcher/Trigger create tasks; worker wrapper executes a noop operator; task status transitions Queued→Running→Completed recorded; heartbeat + reaper kill a stalled task.

### Phase 2: Ingestion
- [ ] **block_follower operator** — Follow chain tip, write to Postgres, handle reorgs. See [operator spec](docs/architecture/operators/block_follower.md).
- [ ] **cryo_ingest operator** — Backfill historical data to S3 Parquet. See [operator spec](docs/architecture/operators/cryo_ingest.md).
- [ ] **Hot storage schema** — blocks, transactions, logs, traces tables in Postgres.
- [ ] **Threshold events** — block_follower emits events for compaction/backfill triggers.
- [ ] **Validate tip freshness** — p95 tip lag target met.

**Acceptance**: block_follower ingests N consecutive blocks with no gaps; reorg simulation rolls back and rewrites hot rows; threshold events fire.

### Phase 3: Query (Hot)
- [ ] **duckdb_query operator (hot only)** — Query Postgres via DuckDB. See [operator spec](docs/architecture/operators/duckdb_query.md).
- [ ] **Query API/CLI** — Basic endpoint for SQL queries.
- [ ] **Authn/authz enforcement** — org_id filtering, role checks.

**Acceptance**: duckdb_query returns correct results for a known dataset; org/role filtering enforced.

### Phase 4: Backfill / Cold Write
- [ ] **cryo_ingest at scale** — Run backfills in parallel with tip ingestion.
- [ ] **Verify cold storage format** — Block-range naming, JSON manifests.

**Acceptance**: cryo_ingest writes correctly named Parquet + JSON manifest.

### Phase 5: Compaction
- [ ] **parquet_compact operator** — Compact hot → cold past finality threshold. See [operator spec](docs/architecture/operators/parquet_compact.md).
- [ ] **Finality threshold config** — Per-chain finality settings.
- [ ] **Optional hot cleanup** — Delete compacted data from Postgres.

**Acceptance**: parquet_compact compacts finalized ranges; optional hot delete works; manifest updated.

### Phase 6: Query (Federated)
- [ ] **duckdb_query federated** — Query across Postgres + S3.
- [ ] **Validate correctness** — Results match across hot/cold boundaries.

**Acceptance**: Federated query returns same totals as separate hot+cold queries on a test dataset.

### Phase 7: Alerting
- [ ] **Alert definitions table + API** — CRUD for alert rules.
- [ ] **alert_evaluate operator** — Pick one runtime first (TS/Py/Rust). See [operator specs](docs/architecture/operators/).
- [ ] **alert_deliver operator** — Email/SMS/webhook delivery. See [operator spec](docs/architecture/operators/alert_deliver.md).
- [ ] **Channel rate limits** — Per-channel throttling.

**Acceptance**: alert_evaluate triggers on a test condition; alert_deliver sends webhook/email; channel rate-limit enforced.

### Phase 8: Integrity
- [ ] **integrity_check operator** — Verify cold storage against canonical chain. See [operator spec](docs/architecture/operators/integrity_check.md).
- [ ] **Scheduled verification** — Periodic integrity job.

**Acceptance**: integrity_check detects a tampered cold file and reports it.

### Deferred
- [ ] User-defined jobs (arbitrary code execution)
- [ ] Multi-chain support beyond Monad
- [ ] Physical tenant isolation (per-org infra)
- [ ] UI/dashboard

---

## In Progress

_None_

---

## Done

_None_
