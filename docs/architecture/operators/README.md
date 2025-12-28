# Operator Catalog

Operators are job implementations packaged as container images. Each operator has a specific purpose, runtime, and contract.

## Index

| Operator | Runtime | Strategy | Description |
|----------|---------|----------|-------------|
| [cryo_ingest](cryo_ingest.md) | Rust | Bulk | Archive historical on-chain data to Parquet |
| [block_follower](block_follower.md) | Rust | Singleton | Follow chain tip, write to hot storage, handle realtime reorgs |
| [integrity_check](integrity_check.md) | Rust | Bulk | Verify cold storage integrity against canonical chain |
| [parquet_compact](parquet_compact.md) | Rust | Bulk | Compact finalized data from hot to cold Parquet |
| [alert_evaluate_ts](alert_evaluate_ts.md) | TypeScript | PerPartition | Evaluate alert conditions (TypeScript) |
| [alert_evaluate_py](alert_evaluate_py.md) | Python | PerPartition | Evaluate alert conditions (Python) |
| [alert_evaluate_rs](alert_evaluate_rs.md) | Rust/Polars | PerPartition | Evaluate alert conditions (Rust) |
| [alert_deliver](alert_deliver.md) | TypeScript | PerPartition | Deliver triggered alerts to channels |
| [duckdb_query](duckdb_query.md) | Rust | Bulk | Execute federated queries across hot/cold |

## Operator Contract

All operators must:

1. **Accept inputs** — Task ID, config, input dataset references
2. **Produce outputs** — Write to storage (S3, Postgres, etc.)
3. **Return metadata** — Status, row counts, output location, timing
4. **Handle failures** — Exit with error code, log details
5. **Support heartbeat** — Worker wrapper handles this; operator code is unaware

## Adding a New Operator

1. Create operator implementation in appropriate runtime
2. Package as container image (one image per operator)
3. Add to ECR
4. Create doc in `operators/`
5. Reference in DAG YAML
