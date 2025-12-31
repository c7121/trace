# Operator Catalog

Operators are job implementations. One container image per runtime.

Runtimes are registered in the Dispatcher runtime registry (image + queue +
capabilities). See [readme.md](../../readme.md) for details.

## Index

| Operator | Runtime | Activation | Execution | Description |
|----------|---------|------------|-----------|-------------|
| [address_labels](address_labels.md) | lambda | source | — | User-defined address labels dataset |
| [block_follower](block_follower.md) | ecs_rust | source | — | Follow chain tip, write to hot storage |
| [liveliness_monitor](liveliness_monitor.md) | lambda | source | — | Detect chain stalls and emit liveliness events |
| [cryo_ingest](cryo_ingest.md) | ecs_rust | reactive | PerPartition | Backfill historical data to S3 |
| [range_aggregator](range_aggregator.md) | ecs_rust | reactive | PerUpdate | Aggregate ordered events into ranges |
| [range_splitter](range_splitter.md) | ecs_rust | reactive | PerPartition | Split ranges into smaller ranges/events |
| [parquet_compact](parquet_compact.md) | ecs_rust | reactive | PerPartition | Compact hot to cold Parquet |
| [integrity_check](integrity_check.md) | ecs_rust | reactive | PerUpdate | Verify cold storage integrity |
| [rpc_integrity_check](rpc_integrity_check.md) | lambda | reactive | PerUpdate | Cross-check RPC providers for divergence |
| [alert_evaluate_ts](alert_evaluate_ts.md) | lambda | reactive | PerUpdate | Evaluate alerts (TypeScript) |
| [alert_evaluate_py](alert_evaluate_py.md) | lambda, ecs_python | reactive | PerUpdate | Evaluate alerts (Python) |
| [alert_evaluate_rs](alert_evaluate_rs.md) | lambda, ecs_rust | reactive | PerUpdate | Evaluate alerts (Rust) |
| [alert_route](alert_route.md) | lambda | reactive | PerUpdate | Route alerts into delivery work items |
| [validator_stats](validator_stats.md) | lambda | source | — | Track validator performance over time |
| [query](duckdb_query.md) | ecs_rust | reactive | PerUpdate | Batch query execution (DuckDB) |

## Operator Contract

1. Receive task (config, cursor, input refs)
2. Execute logic
3. Write output(s) to storage
4. Emit event(s) to Dispatcher
5. Report status

## Runtime Defaults

| Runtime | CPU | Memory | Timeout | Notes |
|---------|-----|--------|---------|-------|
| `lambda` | 0.5 vCPU | 512 MB | 60s | AWS Lambda limits |
| `ecs_rust` | 1 vCPU | 2 GB | 1800s | General-purpose |
| `ecs_python` | 1 vCPU | 4 GB | 1800s | Higher memory for pandas/ML |

Individual operators may override via DAG config (`timeout_seconds`, task definition).

## Adding a New Operator

1. Implement in appropriate runtime image
2. Add to ECR
3. Create doc in `operators/`
4. Reference in DAG YAML
