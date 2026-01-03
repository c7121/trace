# Operator Catalog

Operators are job implementations. v1 exposes a small runtime surface area; language is a packaging detail.


## Index

| Operator | Runtime | Activation | Execution | Description |
|----------|---------|------------|-----------|-------------|
| [address_labels](address_labels.md) | lambda | source | — | User-defined address labels dataset |
| [udf](udf.md) | lambda | reactive | — | Generic UDF execution harness |
| [block_follower](block_follower.md) | ecs_platform | source | — | Follow chain tip, write to hot storage |
| [liveliness_monitor](liveliness_monitor.md) | lambda | source | — | Detect chain stalls and emit liveliness events |
| [cryo_ingest](cryo_ingest.md) | ecs_platform | reactive | PerPartition | Bootstrap historical sync to S3 |
| [range_aggregator](range_aggregator.md) | ecs_platform | reactive | PerUpdate | Aggregate ordered events into ranges |
| [range_splitter](range_splitter.md) | ecs_platform | reactive | PerPartition | Split ranges into smaller ranges/events |
| [parquet_compact](parquet_compact.md) | ecs_platform | reactive | PerPartition | Compact hot to cold Parquet |
| [integrity_check](integrity_check.md) | ecs_platform | reactive | PerUpdate | Verify cold storage integrity |
| [rpc_integrity_check](rpc_integrity_check.md) | lambda | reactive | PerUpdate | Cross-check RPC providers for divergence |
| [alert_evaluate](alert_evaluate.md) | lambda | reactive | PerUpdate | Evaluate alerts (untrusted UDF) |
| [alert_route](alert_route.md) | ecs_platform | reactive | PerUpdate | Route alerts into delivery work items |
| [validator_stats](validator_stats.md) | lambda | source | — | Track validator performance over time |
| [query](duckdb_query.md) | ecs_platform | reactive | PerUpdate | Batch query execution (DuckDB) |

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
| `ecs_platform` | 1 vCPU | 2 GB | 1800s | Trusted platform operators (override per job if needed) |

Individual operators may override via DAG config (`timeout_seconds`, task definition).

## Adding a New Operator

1. Implement in appropriate runtime image
2. Add to ECR
3. Create doc in `operators/`
4. Reference in DAG YAML


## Recipes

These are end-to-end “what should I run?” patterns. They are implemented using built-in operators (and optionally UDFs).

- [Chain liveliness monitoring](liveliness_monitor.md#recipe-chain-liveliness-monitoring)
- [RPC integrity checking](rpc_integrity_check.md#recipe-rpc-integrity-checking)
- [Validator monitoring](validator_stats.md#recipe-validator-monitoring)
