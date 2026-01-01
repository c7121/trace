# ADR 0006: Buffered Postgres Datasets (SQS Buffer + Sink)

## Status
- Accepted (December 2025)

## Decision
- The platform supports a general “buffer → sink” pattern for Postgres-backed datasets:
  - Producers **publish records** to a dataset buffer (SQS).
  - A platform-managed sink service drains the buffer and writes to a Postgres table.
- v1 uses **one SQS queue per buffered dataset** and a sink consumer per dataset queue (shared code, per-dataset config) for isolation and backpressure visibility.
- Dataset schema is declared at deploy time (as part of publishing the dataset in DAG config / registry) and the platform creates the table (and indexes/constraints) during deploy/startup.
- v1 uses **SQS** for dataset buffers (Kinesis is deferred until real throughput/ordering problems are observed).

## Context
- Some datasets are naturally **multi-writer** (many jobs produce the same event stream), e.g., alerts, integrity signals, monitoring events.
- Some datasets should be written via a platform-managed sink (not direct Postgres writes from user code).
- We want buffering, backpressure visibility, and decoupling between producers and Postgres writes.
- We do not want to grant arbitrary jobs direct Postgres write credentials or DDL privileges.

## Why
- **NiFi-like wiring**: multiple DAG nodes can feed a shared buffer which fans into sinks.
- **Security**: producers get only `SendMessage` permissions; the sink holds Postgres write credentials.
- **Backpressure**: queue depth/age becomes the explicit buffer health metric.
- **Operational simplicity (v1)**: SQS is already part of the stack and works with VPC endpoints.

## Notes (EIP mapping)
- Buffer: **Message Channel** (Queue Channel).
- Sink: **Service Activator** that persists to a **Message Store** (Postgres).

## Consequences
- Dataset update events for buffered datasets are emitted **by the sink** after commit (not directly by producers).
- Buffered Postgres dataset schema is managed by the platform and updated via deploy-time migrations (DAG update + controlled migration), not runtime “first writer creates table”.
- Schema evolution for derived datasets is a first-class ETL concern and is handled via dataset versions and output commits (see [data_versioning.md](../data_versioning.md)).
- Multi-writer datasets are supported: multiple producers can publish to the same buffer and rely on sink-side idempotency keys / unique constraints.
- Buffers use an SQS DLQ (redrive policy). Messages that exceed the max receive count require manual inspection and replay.
