# Metadata and Lineage

How the platform tracks data provenance, versioning, and job history.

## What the System Tracks

| Entity | Description |
|--------|-------------|
| **Assets** | Named, versioned data outputs |
| **Lineage** | Full graph of which jobs produced which assets from which inputs |
| **Materialization metadata** | When, how long, row counts, custom metadata |
| **Partitions** | Logical slices (by date, block range, etc.) |
| **Schema** | Column names, types, structure of each asset |
| **Run history** | Who initiated, parameters, config, success/failure, logs |

## Versioning and Rollback

| Data Type | Behavior |
|-----------|----------|
| **Core chain data** | Immutable in cold storage (S3 Parquet) after finality; hot storage (Postgres) is mutable to handle reorgs at chain tip |
| **Derived assets** | Versioned; overwrites create new versions, previous versions retained |
| **PII/user data** | Mutable; deletion and redaction must be possible |

**Refresh propagation**: Derived datasets are refreshed whenever upstream datasets are refreshed.

**Rollback**: Users can restore or reprocess from a previous asset version.

## Debugging and Iteration

- **Inspectable outputs**: published datasets are discoverable/queryable; internal edges can be made queryable by publishing (see [ADR 0008](../architecture/adr/0008-dataset-registry-and-publishing.md))
- **Error visibility**: failed jobs expose error messages, stack traces, logs
- **Edit and re-run**: users can modify a job/node and re-run downstream jobs
- **Selective re-run**: re-run a single job without re-running upstream

## Related

- [data_versioning.md](../architecture/data_versioning.md) — partition versioning, incremental processing
- [readme.md](../readme.md) — data model schema
