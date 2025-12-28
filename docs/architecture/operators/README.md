# Operator Catalog

Operators are job implementations. One container image per runtime.

Runtimes are registered in the Dispatcher runtime registry (image + queue +
capabilities). See [overview.md](../overview.md) for details.

## Index

| Operator | Runtime | Activation | Execution | Description |
|----------|---------|------------|-----------|-------------|
| [block_follower](block_follower.md) | ecs_rust | source | — | Follow chain tip, write to hot storage |
| [cryo_ingest](cryo_ingest.md) | ecs_rust | reactive | PerPartition | Backfill historical data to S3 |
| [parquet_compact](parquet_compact.md) | ecs_rust | reactive | Bulk | Compact hot to cold Parquet |
| [integrity_check](integrity_check.md) | ecs_rust | reactive | Bulk | Verify cold storage integrity |
| [alert_evaluate_ts](alert_evaluate_ts.md) | lambda | reactive | PerUpdate | Evaluate alerts (TypeScript) |
| [alert_evaluate_py](alert_evaluate_py.md) | ecs_python | reactive | PerUpdate | Evaluate alerts (Python) |
| [alert_evaluate_rs](alert_evaluate_rs.md) | ecs_rust | reactive | PerUpdate | Evaluate alerts (Rust) |
| [alert_deliver](alert_deliver.md) | lambda | reactive | PerUpdate | Deliver alerts to channels |
| [query](duckdb_query.md) | ecs_rust | reactive | Bulk | Batch query execution (DuckDB) |

## Operator Contract

1. Receive task (config, cursor, input refs)
2. Execute logic
3. Write output to storage
4. Emit event to Dispatcher
5. Report status

## Adding a New Operator

1. Implement in appropriate runtime image
2. Add to ECR
3. Create doc in `operators/`
4. Reference in DAG YAML
