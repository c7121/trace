# Data Ingestion

How data enters the platform — onchain and offchain, real-time and historical.

## Overview

- System ingests onchain data continuously (real-time at chain tip) and via backfills (historical ranges)
- System can ingest offchain data feeds
- Ingestion is a job type — pluggable, not hardcoded to a specific tool

## onchain Ingestion

| Mode | Operator | Storage | Use Case |
|------|----------|---------|----------|
| Real-time | `block_follower` | Postgres (hot) | Chain tip, reorg handling |
| Historical | `cryo_ingest` | S3 Parquet (cold) | Backfills, archive |

### Requirements

- Archive historical onchain data (e.g., Cryo datasets to Parquet)
- Ingest recent blocks at high frequency (e.g., 400ms block time on Monad)
- May use streaming formats (Avro) or transactional stores (Postgres)
- Unified query across historical and recent data (via DuckDB federation)
- Reorg detection and correction

## offchain Ingestion

offchain feeds (price data, labels, external APIs) enter as source jobs at DAG entry points.

External data ingestion happens at DAG entry points (sources), not mid-job.

## Related

- [block_follower operator](../architecture/operators/block_follower.md)
- [cryo_ingest operator](../architecture/operators/cryo_ingest.md)
- [parquet_compact operator](../architecture/operators/parquet_compact.md) — finalize and compact hot → cold
- [integrity_check operator](../architecture/operators/integrity_check.md) — defense-in-depth verification and repair
- [data_versioning.md](../architecture/data_versioning.md) — reorg handling
