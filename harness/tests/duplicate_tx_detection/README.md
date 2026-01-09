# Duplicate Transaction Detection Test

Verifies that the DuckDB query correctly detects transaction hashes appearing in multiple blocks - a data integrity issue that could indicate reorgs, RPC bugs, or ingestion errors.

## Query

```sql
SELECT 
  transaction_hash,
  MIN(block_number) AS first_block,
  MAX(block_number) AS last_block,
  COUNT(DISTINCT block_number) AS num_blocks
FROM read_parquet('*.parquet')
GROUP BY transaction_hash
HAVING COUNT(DISTINCT block_number) > 1;
```

## Test Data

Real transaction data from Monad mainnet (chain_id=143):

| File | Blocks | Description |
|------|--------|-------------|
| `tx_45000.parquet` | 45000-45999 | Contains txs in blocks 45216, 45463 |
| `tx_46000.parquet` | 46000-46999 | Contains tx in block 46336 |
| `tx_fake_duplicate.parquet` | 99999 | Same txs as tx_45000 with fake block_number |

The fake file simulates a data integrity issue where transactions appear in multiple blocks.

## Running the Test

```bash
./run_test.sh
```

Expected output:
- Query returns 2 rows (the duplicated transactions)
- Exit code 0 with "PASS" message

## Running Against Live Data

To scan all synced transactions in MinIO:

```bash
docker run --rm --network=harness_default datacatering/duckdb:v1.1.3 -c "
INSTALL httpfs; LOAD httpfs;
SET s3_region='us-east-1';
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
FROM read_parquet('s3://trace-harness/cryo/143/6f8b4b60-1e53-51d1-807c-93fad2c5ac95/**/*.parquet', hive_partitioning=false)
GROUP BY transaction_hash
HAVING COUNT(DISTINCT block_number) > 1;
"
```

Replace `6f8b4b60-1e53-51d1-807c-93fad2c5ac95` with the transactions dataset UUID for your chain.
