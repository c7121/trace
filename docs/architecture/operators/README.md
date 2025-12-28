# Operator Catalog

Operators are job implementations. One container image per operator type.

## Index

| Operator | Operator Type | Trigger | Execution | Description |
|----------|---------------|---------|-----------|-------------|
| [block_follower](block_follower.md) | ingest | none | — | Follow chain tip, write to hot storage |
| [cryo_ingest](cryo_ingest.md) | ingest | upstream | PerPartition | Backfill historical data to S3 |
| [wire_tap](wire_tap.md) | virtual | upstream | — | Copy events to secondary destination |
| [parquet_compact](parquet_compact.md) | polars | upstream | Bulk | Compact hot → cold Parquet |
| [integrity_check](integrity_check.md) | polars | upstream | Bulk | Verify cold storage integrity |
| [alert_evaluate_ts](alert_evaluate_ts.md) | lambda | upstream | PerUpdate | Evaluate alerts (TypeScript) |
| [alert_evaluate_py](alert_evaluate_py.md) | python | upstream | PerUpdate | Evaluate alerts (Python) |
| [alert_evaluate_rs](alert_evaluate_rs.md) | polars | upstream | PerUpdate | Evaluate alerts (Rust) |
| [alert_deliver](alert_deliver.md) | lambda | upstream | PerUpdate | Deliver alerts to channels |
| [duckdb_query](duckdb_query.md) | polars | upstream | Bulk | Execute federated queries |

## Operator Contract

1. Receive task (config, cursor, input refs)
2. Execute logic
3. Write output to storage
4. Emit event to Dispatcher
5. Report status

## Adding a New Operator

1. Implement in appropriate operator type image
2. Add to ECR
3. Create doc in `operators/`
4. Reference in DAG YAML
