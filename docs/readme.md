# Trace Platform

An ETL orchestration platform for blockchain research and operations.

## What is Trace?

A general-purpose ETL orchestration system: safe, reliable, and extensible.

**Users:** Analysts and researchers · DeFi teams · Security professionals

**User stories** — as an analyst or researcher, I can:
- **Curate** onchain data — select, filter, and organize datasets from blockchain networks
- **Combine** onchain data with offchain feeds — enrich blockchain data with external sources
- **Enrich** data — add labels, annotations, and computed fields (both real-time and retroactive)
- **Alert** on data — define conditions and receive notifications on historical and live data
- **Analyze** data — run summaries, aggregations, and models across the dataset
- **Access both historical and real-time data** — seamless queries across full history and chain tip

**Goals:**
- **Safe** — least privilege access; secrets managed securely; full audit trail
- **Reliable** — no silent data loss; system recovers gracefully from failures
- **Extensible** — variety of data in (onchain, offchain, batch, stream, push, pull); variety of operations out (query, enrich, alert, model)

**Non-goals:** Ultra-low-latency trading · On-prem deployment · Multi-tenancy in v1

**Assumptions:** AWS (portable design) · Monad-first (EVM-compatible; multi-chain ready) · IaC-only provisioning

---

## Concepts

### Design Principles

1. **Everything is a job** — Streaming services, batch transforms, checks
2. **Everything produces assets** — Postgres tables, S3 Parquet, any URI
3. **Workers are dumb** — Receive task, execute, report result
4. **YAML is source of truth** — Definitions in git, state in Postgres state
5. **Single dispatcher service** — Simple, stateless, restartable

### Job Types

| Type | Purpose | Example |
|------|---------|---------|
| Ingest | Pull data from onchain or offchain sources | `block_follower`, `cryo_ingest` |
| Transform | Alter, clean, reshape data | decode logs |
| Combine | Join or merge datasets | onchain + offchain |
| Enrich | Add labels, annotations, computed fields | address tagging |
| Summarize | Aggregate, roll up, compute metrics | daily volumes |
| Validate | Check invariants, data quality | `integrity_check` |
| Alert | Evaluate conditions, route notifications | `alert_evaluate`, `alert_route` |

### Glossary

| Term | Definition |
|------|------------|
| Operator | Job implementation (e.g., `block_follower`, `alert_evaluate`) |
| Activation | `source` (emits events) or `reactive` (runs from tasks) |
| Source | Job with `activation: source` — maintains connections, emits events |
| Asset | Output of a job — Parquet file, table rows |
| Partition | A subset of an asset (e.g., blocks 0-10000) |
| Runtime | Execution environment: `lambda`, `ecs_platform`, `dispatcher` (v1); `ecs_udf` is deferred to v2 |

---

## Architecture

Canonical C4 diagrams live in [c4.md](architecture/c4.md) (L1 System Context, L2 Container View).

### Key characteristics

- **Multi-runtime** — Rust, Python, TypeScript (v1)
- **Asset-based lineage** — Everything produces trackable assets
- **Flexible partitioning** — Data-driven, not static time-based
- **Config-as-code** — DAGs defined in YAML, version controlled
- **Single-tenant v1** — `org_id` scoping throughout for future multi-tenant expansion

### Storage

- **Postgres state** — orchestration metadata (multi-AZ, PITR)
- **Postgres data** — hot/mutable datasets (recent chain ranges, alert tables)
- **S3 Parquet** — cold/immutable datasets and exported results
- **DuckDB** — federates queries across Postgres data and S3

See [db_boundaries.md](architecture/db_boundaries.md) for cross-database constraints.

### Security

- **Trust split**: Platform Workers run trusted operators; untrusted user code runs only via Lambda UDF runner
- **Secrets**: AWS Secrets Manager → injected at launch; untrusted code never calls Secrets Manager
- **Egress**: No direct internet from job containers; all external calls via Delivery Service or RPC Egress Gateway

Full model: [security_model.md](standards/security_model.md)

---

## Reading Order

For developers and agents implementing features:

1. **[invariants.md](architecture/invariants.md)** — Correctness guarantees (read first)
2. **[contracts.md](architecture/contracts.md)** — Wire formats, JWT claims, API fencing
3. **[task_lifecycle.md](architecture/task_lifecycle.md)** — Leasing, retries, outbox
4. **Relevant spec** in [specs/](specs/) — Feature-specific surface and acceptance criteria
5. **Relevant ADR** in [adr/](adr/) — Decision rationale if changing boundaries

---

## Documentation Map

| Area | Documents |
|------|-----------|
| Architecture | [architecture/](architecture/), [ADRs](adr/) |
| Containers | [containers/](architecture/containers/) |
| Data model | [data_model/](architecture/data_model/) |
| Operators | [operators/](architecture/operators/) |
| Specs | [specs/](specs/) |
| Deploy | [deploy/](deploy/) |
| Standards | [standards/](standards/) |

When updating docs, follow [docs_hygiene.md](standards/docs_hygiene.md).

---

## References

- [cryo GitHub](https://github.com/paradigmxyz/cryo)
- [DuckDB Documentation](https://duckdb.org/docs/)
- [AWS ECS Autoscaling](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/service-auto-scaling.html)
