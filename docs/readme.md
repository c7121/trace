# Trace Platform

Architecture overview for Trace: an ETL orchestration platform for blockchain research and operations.

## Overview

A general-purpose ETL orchestration system designed for:

- **Multi-runtime support** — Rust, Python, TypeScript (v1); additional runtimes are deferred (see backlog)
- **Asset-based lineage** — Everything produces trackable assets
- **Flexible partitioning** — Data-driven, not static time-based
- **Source jobs** — Long-running services with `activation: source` (e.g., blockchain followers)
- **Config-as-code** — DAGs defined in YAML, version controlled

See [plan.md](plan/plan.md) for the recommended build order and [backlog.md](plan/backlog.md) for deferred items.

### Design Principles

1. **Everything is a job** — Streaming services, batch transforms, checks
2. **Everything produces assets** — Postgres tables, S3 Parquet, any URI
3. **Workers are dumb** — Receive task, execute, report result
4. **YAML is source of truth** — Definitions in git, state in Postgres state
5. **Single dispatcher service** — Simple, stateless, restartable

### Tenancy Model

> **v1 is single-tenant.** The architecture includes `org_id` scoping throughout (jobs, tasks, data, queries) to support future multi-tenant expansion, but v1 deploys as a single-org instance. Multi-tenancy (shared infrastructure with logical isolation) and physical tenant isolation (per-org deployments) are deferred. See [backlog.md](plan/backlog.md).

### Job Characteristics

- **Containerized**: jobs run as containers or services, called remotely (not co-located)
- **Polyglot**: any runtime — Rust, Python, TypeScript, etc. — packaged as a container
- **Standard contract**: jobs receive inputs, produce outputs, return metadata
- **Composable**: jobs can depend on outputs of other jobs, forming DAGs

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

Canonical C4 diagrams live in [c4.md](architecture/c4.md):

- **C4 L1 (System Context)**
- **C4 L2 (Container View)** — includes Platform Workers and the Lambda UDF runner (v1), Dispatcher credential minting, Delivery Service, and egress gateways

This `docs/readme.md` keeps the architecture overview concise; use the C4 page for diagrams and component boundaries.


### Storage

**Storage:** Postgres state holds orchestration metadata (multi-AZ, PITR). Postgres data and S3 are used for job data: Postgres data is typically used for hot/mutable datasets (e.g., recent chain ranges, alert tables), while S3 Parquet is used for cold/immutable datasets and exported results. The "hot" vs "cold" split is a **naming convention** used by operators like `block_follower` and `parquet_compact`, not a separate storage engine. DuckDB federates across both.

See [db_boundaries.md](architecture/db_boundaries.md) for hard invariants and cross-database constraints.


### Deep Dives

- C4 diagrams: [c4.md](architecture/c4.md)
- End-to-end flow: [event_flow.md](architecture/event_flow.md)
- Task lifecycle: [task_lifecycle.md](architecture/task_lifecycle.md)
- Operations (targets, invariants, failure drills): [operations.md](standards/operations.md)
- Orchestration internals: [dispatcher.md](architecture/containers/dispatcher.md)
- Execution model: [workers.md](architecture/containers/workers.md)
- Database boundaries: [db_boundaries.md](architecture/db_boundaries.md)
- Query federation: [query_service.md](architecture/containers/query_service.md)
- Scoped data access: [dispatcher.md#credential-minting](architecture/containers/dispatcher.md#credential-minting)
- Outbound egress: [delivery_service.md](architecture/containers/delivery_service.md), [rpc_egress_gateway.md](architecture/containers/rpc_egress_gateway.md)
- User API surface: [user_api_contracts.md](architecture/user_api_contracts.md)
- Task/event schemas: [contracts.md](architecture/contracts.md)


## Documentation Map

| Area | Documents |
|------|-----------|
| Architecture | [architecture index](architecture/README.md), [ADRs](adr/) |
| Containers | [containers](architecture/containers/) |
| Data model | [data model](architecture/data_model/) |
| Operators | [operator catalog](architecture/operators/README.md) |
| Specs | [spec index](specs/README.md) |
| Deploy | [deployment profiles](deploy/deployment_profiles.md), [infrastructure](deploy/infrastructure.md), [monitoring](deploy/monitoring.md) |
| Standards | [standards](standards/) |
| Use cases | [operator recipes](architecture/operators/README.md#recipes) |
| Planning | [planning](plan/) |

When updating docs or diagrams, follow [docs_hygiene.md](standards/docs_hygiene.md).

## Security

- **Trust split**: Platform Workers run trusted operators; untrusted user code executes only via the platform-managed Lambda UDF runner (v1).
- **Secrets**: stored in AWS Secrets Manager and injected into ECS/Lambda at launch; untrusted code does not call Secrets Manager.
- **Egress**: job containers have no direct internet egress. External calls go only through platform egress services (Delivery Service, RPC Egress Gateway).
- **Roles**: dispatcher, platform workers, lambda udf runner, query service, delivery service, rpc egress gateway.

The full isolation model and threat assumptions live in [security_model.md](standards/security_model.md).

## References


- [cryo GitHub](https://github.com/paradigmxyz/cryo)
- [DuckDB Documentation](https://duckdb.org/docs/)
- [AWS ECS Autoscaling](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/service-auto-scaling.html)
