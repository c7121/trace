# duckdb_query

Execute federated queries across hot and cold storage.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | Rust |
| **Execution Strategy** | Bulk |
| **Image** | `duckdb_query:latest` |

## Description

Executes SQL queries that span both hot storage (Postgres) and cold storage (S3 Parquet) using DuckDB's federation capabilities. Returns unified results as if querying a single database.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `query` | config | SQL query to execute |
| `output_format` | config | Result format (parquet, json, csv) |
| `output_path` | config | Where to write results |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Query results | `s3://{bucket}/results/{job_id}/` | Parquet/JSON/CSV |
| Query metadata | `postgres://query_results` | Rows |

## Triggers

- **Manual**: User-initiated queries
- **Dependency**: Downstream job needs materialized view
- **Cron**: Scheduled report generation

## Behavior

- Parses SQL query
- Attaches Postgres (hot) and S3/Parquet (cold) as DuckDB sources
- Executes federated query
- Writes results to specified output
- Records execution metadata (rows, duration, bytes scanned)

## Query Capabilities

| Feature | Support |
|---------|---------|
| JOIN across hot/cold | ✅ |
| Aggregations | ✅ |
| Window functions | ✅ |
| Parquet pushdown | ✅ |
| Postgres pushdown | ✅ |

## Dependencies

- DuckDB with postgres_scanner and parquet extensions
- Postgres read access
- S3 read access to cold storage
- S3 write access for results

## Example DAG Config

```yaml
- name: daily_summary
  job_type: Transform
  execution_strategy: Bulk
  runtime: Rust
  entrypoint: duckdb_query
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
  output_dataset: daily_summaries
  timeout_seconds: 3600
```

## Notes

- `unified_transactions` is a virtual table spanning hot + cold
- DuckDB handles partition pruning automatically
- Large result sets should use Parquet output, not JSON
