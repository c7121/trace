# query (DuckDB)

Execute federated queries across hot and cold storage.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_rust` |
| **Activation** | `reactive` |
| **Execution Strategy** | Bulk |
| **Image** | `duckdb_query:latest` |

## Description

Executes SQL queries that span both hot storage (Postgres) and cold storage (S3 Parquet)
using DuckDB's federation capabilities. This is the batch query operator; interactive
queries use the query service endpoint.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `query` | config | SQL query to execute |
| `output_format` | config | Result format (parquet, json, csv) |
| `output_path` | config | Where to write results |
| `query_id` | config | Query execution id (matches `query_results.id` when invoked by Query Service) |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Query results | `s3://{bucket}/results/{org_id}/{query_id}/` (default) | Parquet/JSON/CSV |
| Query metadata | `postgres://query_results` | Rows |

## Execution

- **Manual**: User-initiated queries
- **Dependency**: Downstream job needs materialized view
- **Cron**: Scheduled report generation

## Behavior

- Parses SQL query
- Attaches Postgres (hot) and S3/Parquet (cold) as DuckDB sources
- Executes federated query
- Writes results to specified output
- Records execution metadata to `query_results` (rows, duration, bytes scanned, output location)

## Query Capabilities

See [Query Capabilities](../query_service.md#query-capabilities) for the supported SQL feature set.

## Dependencies

- DuckDB with postgres_scanner and parquet extensions
- Postgres read access
- S3 read access to cold storage
- S3 write access for results
- Postgres write access to `query_results` (see [query_service.md](../query_service.md#query-results))

## Example DAG Config

```yaml
- name: daily_summary
  activation: reactive
  runtime: ecs_rust
  operator: query
  execution_strategy: Bulk
  config:
    query: |
      SELECT date_trunc('day', block_timestamp) as day,
             count(*) as tx_count,
             sum(value) as total_value
      FROM unified_transactions
      GROUP BY 1
    output_format: parquet
    output_path: s3://bucket/summaries/daily/
  input_datasets: [hot_transactions, cold_transactions]
  output_datasets: [daily_summaries]
  timeout_seconds: 3600
```

## Notes

- `unified_transactions` is a virtual table spanning hot + cold
- DuckDB handles partition pruning automatically
- Large result sets should use Parquet output, not JSON
