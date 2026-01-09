# Data Verification Runbook

Procedures for verifying data integrity in trace pipelines.

## Duplicate Transaction Detection

Identifies transaction hashes appearing in multiple blocks - a sign of data corruption or sync issues.

### Query

```sql
SELECT
  transaction_hash,
  MIN(block_number) AS first_block,
  MAX(block_number) AS last_block,
  COUNT(DISTINCT block_number) AS num_blocks
FROM transactions  -- or read_parquet(...) for file-based queries
GROUP BY transaction_hash
HAVING COUNT(DISTINCT block_number) > 1
ORDER BY num_blocks DESC;
```

**Expected result:** 0 rows. Any rows indicate duplicate transaction data.

### Running in Trace Lite (local/harness)

Use DuckDB to query parquet files in MinIO:

```bash
# From repo root, with harness running (docker compose up -d)
docker run --rm --network=harness_default datacatering/duckdb:v1.1.3 -c "
  SET s3_endpoint='minio:9000';
  SET s3_access_key_id='trace';
  SET s3_secret_access_key='tracepassword';
  SET s3_use_ssl=false;
  SET s3_url_style='path';

  SELECT
    transaction_hash,
    MIN(block_number) AS first_block,
    MAX(block_number) AS last_block,
    COUNT(DISTINCT block_number) AS num_blocks
  FROM read_parquet(
    's3://trace-data/datasets/<DATASET_UUID>/data/**/*.parquet',
    hive_partitioning=false
  )
  GROUP BY transaction_hash
  HAVING COUNT(DISTINCT block_number) > 1;
"
```

To find the dataset UUID for a chain's transactions:

```bash
docker run --rm --network=harness_default datacatering/duckdb:v1.1.3 -c "
  SET s3_endpoint='minio:9000';
  SET s3_access_key_id='trace';
  SET s3_secret_access_key='tracepassword';
  SET s3_use_ssl=false;
  SET s3_url_style='path';

  SELECT id, name FROM read_parquet('s3://trace-data/datasets/*/metadata.parquet')
  WHERE name LIKE '%transactions%';
"
```

### Running in Production

Query the data warehouse directly:

```sql
-- Adjust table name to match your deployment
SELECT
  transaction_hash,
  MIN(block_number) AS first_block,
  MAX(block_number) AS last_block,
  COUNT(DISTINCT block_number) AS num_blocks
FROM blockchain.transactions
WHERE chain_id = <CHAIN_ID>
GROUP BY transaction_hash
HAVING COUNT(DISTINCT block_number) > 1
ORDER BY num_blocks DESC
LIMIT 100;
```

### Harness Test Fixture

A self-contained test with mock data lives in:

- [harness/diagnostics/duplicate_tx_detection/](../../harness/diagnostics/duplicate_tx_detection/)

This includes real parquet files plus a synthetic duplicate for validating the query catches issues:

```bash
cd harness/diagnostics/duplicate_tx_detection
./run_test.sh
```

The test should output `PASS` and show 2 detected duplicates (the intentional fakes).

---

## Block Gap Detection

*TODO: Add procedure for detecting missing blocks in sync ranges.*

## Schema Validation

*TODO: Add procedure for validating parquet schema consistency.*
