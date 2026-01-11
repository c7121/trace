# Operator specs

Operator docs define the DAG surface area for built-in operators: operator name, config semantics, inputs and outputs, and behavioral expectations.

Most operators described here are planned. Each operator doc includes a `Status:` line indicating whether it is implemented and in which profiles.

## Status legend

- `planned`: surface documented, not implemented in code yet.
- `implemented (Lite)`: implemented in Trace Lite and harness.
- `implemented (AWS)`: implemented in AWS profile.
- `implemented (Lite,AWS)`: implemented in both Lite and AWS profiles.

## Index

| Operator | Status | Runtime | Activation | Execution | Description |
|----------|--------|---------|------------|-----------|-------------|
| [address_labels](address_labels.md) | planned | lambda | source | - | User-defined address labels dataset |
| [udf](udf.md) | planned | lambda | reactive | PerUpdate or PerPartition | Generic UDF execution harness |
| [block_follower](block_follower.md) | planned | ecs_platform | source | - | Follow chain tip, write to hot storage |
| [liveliness_monitor](liveliness_monitor.md) | planned | lambda | source | - | Detect chain stalls and emit liveliness events |
| [cryo_ingest](cryo_ingest.md) | implemented (Lite) | ecs_platform | reactive | PerPartition | Bootstrap historical sync to object storage |
| [range_aggregator](range_aggregator.md) | planned | ecs_platform | reactive | PerUpdate | Aggregate ordered events into ranges |
| [range_splitter](range_splitter.md) | planned | ecs_platform | reactive | PerPartition | Split ranges into smaller ranges or events |
| [parquet_compact](parquet_compact.md) | planned | ecs_platform | reactive | PerPartition | Compact hot to cold Parquet |
| [integrity_check](integrity_check.md) | planned | ecs_platform | reactive | PerUpdate | Verify cold storage integrity |
| [rpc_integrity_check](rpc_integrity_check.md) | planned | lambda | reactive | PerUpdate | Cross-check RPC providers for divergence |
| [alert_evaluate](alert_evaluate.md) | planned | lambda | reactive | PerUpdate | Evaluate alerts (untrusted UDF) |
| [alert_route](alert_route.md) | planned | ecs_platform | reactive | PerUpdate | Route alerts into delivery work items |
| [validator_stats](validator_stats.md) | planned | lambda | source | - | Track validator performance over time |
| [query](duckdb_query.md) | planned | ecs_platform | reactive | PerUpdate | Batch query execution (DuckDB) |

## Recipes

See [examples/README.md](../../examples/README.md) for operator recipes and end-to-end runs.

## Related

- DAG schema: [dag_configuration.md](../dag_configuration.md)
- Architecture container tour: [c4.md](../../architecture/c4.md)
