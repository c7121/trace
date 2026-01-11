# ADRs

Architecture Decision Records capture durable decisions and rationale.

Template: [_template.md](_template.md)

## Index

| ADR | Summary |
|-----|---------|
| [0001-orchestrator.md](0001-orchestrator.md) | Custom orchestrator (Dispatcher + workers) with YAML-defined DAGs |
| [0002-networking.md](0002-networking.md) | No internet egress by default; outbound via platform egress services |
| [0003-udf-bundles.md](0003-udf-bundles.md) | Lambda-style zip bundles for untrusted UDF execution |
| [0004-alert-event-sinks.md](0004-alert-event-sinks.md) | Multi-writer alert event sink and delivery work items |
| [0005-query-results.md](0005-query-results.md) | Platform-managed `query_results` table for query auditing and outputs |
| [0006-buffered-postgres-datasets.md](0006-buffered-postgres-datasets.md) | Buffered dataset pattern: object storage batch + queue + sink |
| [0007-input-edge-filters.md](0007-input-edge-filters.md) | `where` input edge filters as structured maps |
| [0008-dataset-registry-and-publishing.md](0008-dataset-registry-and-publishing.md) | Dataset registry and `publish:` mapping from job outputs to names |
| [0009-atomic-cutover-and-query-pinning.md](0009-atomic-cutover-and-query-pinning.md) | Atomic cutover/rollback for versioned datasets; query pinning |
| [0010-trace-core-error-contract.md](0010-trace-core-error-contract.md) | trace-core error type and result alias (no `anyhow::Result` surface) |
