# Functional Specification (Draft)

## Core Concept: Jobs

Jobs are the universal primitive. All operations — ingestion, transformation, enrichment, validation, alerting — are jobs.

### Job characteristics
- **Containerized**: jobs run as containers or services, called remotely (not co-located).
- **Polyglot**: any runtime — Scala, R, Python, etc. — packaged as a container.
- **Standard contract**: jobs receive inputs, produce outputs, and return metadata for the system to track.
- **Composable**: jobs can depend on outputs of other jobs, forming DAGs.

### Job activation
- **Source jobs**: emit events directly. Source kinds include `always_on`, `cron`, `webhook`, and `manual`.
- **Reactive jobs**: run from Dispatcher tasks when events arrive for `input_datasets`.

### Job types
- **Ingest**: pull data from on-chain (real-time, backfill) or off-chain sources.
- **Transform**: alter, clean, reshape data.
- **Combine**: join or merge datasets (on-chain + off-chain, multiple chains, etc.).
- **Enrich**: add labels, annotations, computed fields.
- **Summarize**: aggregate, roll up, compute metrics.
- **Validate**: check invariants, data quality, consistency.
- **Alert delivery**: relay alert notifications over configured channels.

### Known job requirements
- Archive historical on-chain data (e.g., Cryo datasets to Parquet).
- Ingest recent blocks at high frequency (e.g., 400ms block time on Monad); may use streaming formats (Avro) or transactional stores (Postgres).
- Unified query across historical and recent data (e.g., via DuckDB federation).
- Reorg detection and correction.

## Data Ingestion

- System ingests on-chain data continuously (real-time at chain tip) and via backfills (historical ranges).
- System can ingest off-chain data feeds.
- Ingestion is a job type — pluggable, not hardcoded to a specific tool.

## Alerting

- **Definitions**: users create alert rules (stored as config/rows) specifying conditions on data.
- **Evaluation**: alerts can run on real-time incoming data or historical data.
- **Delivery**: separate job relays alerts over appropriate channels (email, SMS, webhook, etc.).

## Data Access and Querying

- **Query interface**: users query data via SQL or API; spans historical and recent data seamlessly.
- **Ad-hoc exploration**: interactive queries for analysis and debugging.
- **Downloads**: users can export query results or datasets.
- **Summarization and modeling**: users can run aggregations, summaries, and models against the data.
- **Saved queries/views**: users can save and share queries for reuse.
- **Discovery and catalog**: users can browse and search available datasets, jobs, and assets within their org.

## Metadata and Lineage

System tracks:
- **Assets**: named, versioned data outputs.
- **Lineage**: full graph of which jobs produced which assets from which inputs.
- **Materialization metadata**: when, how long, row counts, custom metadata.
- **Partitions**: logical slices (by date, block range, etc.).
- **Schema**: column names, types, structure of each asset.
- **Run history**: who initiated, parameters, config, success/failure, logs.

### Versioning and rollback
- **Core chain data**: immutable in cold storage (S3 Parquet) after finality; hot storage (Postgres) is mutable to handle reorgs at chain tip.
- **Derived assets**: versioned; overwrites create new versions, previous versions retained.
- **Refresh propagation**: derived datasets are refreshed whenever upstream datasets are refreshed.
- **PII/user data**: mutable; deletion and redaction must be possible.
- **Rollback**: users can restore or reprocess from a previous asset version.

### Debugging and iteration
- **Inspectable outputs**: every DAG node's output is viewable.
- **Error visibility**: failed jobs expose error messages, stack traces, logs.
- **Edit and re-run**: users can modify a job/node and re-run downstream jobs.
- **Selective re-run**: re-run a single job without re-running upstream.

## Storage

- Outputs written to object storage (S3), databases (Postgres), or other destinations.
- Jobs can write anywhere, provided downstream jobs can access the output as input.
- Filenames/paths follow conventions defined per dataset or job.
- Manifests emitted per job run for integrity verification.
- External data ingestion happens at DAG entry points (sources), not mid-job.

## Access Control

### Hierarchy
- **Global**: platform-wide settings and shared data.
- **Org**: organization-scoped data and jobs; one tenant per org.
- **Role**: permissions within an org (reader, writer, admin).
- **User**: individual-level access and private data.

### Tenant Isolation
- Each org is an isolated tenant.
- **Logical isolation** (default): `org_id` filtering on all queries, per-org quotas, rate limits.
- **Physical isolation** (optional): separate VPC, ECS cluster, RDS instance for compliance/enterprise.
- One org's workload cannot starve another's resources.
- Cross-org sharing is explicit grants, not shared infrastructure.

### Behaviors
- Users can be granted access to another org's data (cross-org sharing).
- All actions (job execution, data access, config changes) require authn/authz.
- Private user data (e.g., labels, enrichments) is access-controlled.

### PII and Sensitive Data
- Address labeling and similar enrichments may constitute PII.
- PII access is logged: who accessed, what they accessed, when.
- PII storage is isolated and access-restricted.
- Audit logs for PII access are retained and reviewable.

### Identity
- Users authenticate via centralized identity provider (OIDC/SAML); pluggable.

## System Behaviors

- **Rate limits**: respect external provider limits; back off when throttled.
- **Retries**: transient failures are retried; persistent failures go to dead-letter.
- **Failover**: detect issues and reroute to healthy alternatives.
- **Idempotency**: re-running a job for the same inputs produces the same outputs.
- **Data integrity**: verify completeness; detect and recover gaps.

## Job Security Model

See [security.md](security.md) for the full job security model including:
- Threat model
- Container isolation
- Network isolation
- Resource limits
- Data access control
- Credential handling
- Audit and monitoring

## Observability

- **Metrics**: system emits job success/error rates, latency, throughput, cost signals.
- **Logs/traces**: correlated across job runs; secrets redacted.
- **System alerts**: system alerts on failures and anomalies (separate from user-defined alerts).
