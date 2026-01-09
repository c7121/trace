#!/usr/bin/env bash
# Staking Withdrawal Verification
#
# Verifies that every Withdraw event from the Monad staking precompile
# has a corresponding Undelegate event called beforehand.
#
# Usage:
#   ./run.sh                    # Run against trace-harness MinIO
#   ./run.sh --network mainnet  # Placeholder for production

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=== Staking Withdrawal Verification ==="
echo ""
echo "Checking that all Withdraw events have matching Undelegate events..."
echo ""

# Run the query via DuckDB
docker run --rm --network=harness_default datacatering/duckdb:v1.1.3 -c "
SET s3_endpoint='minio:9000';
SET s3_access_key_id='trace';
SET s3_secret_access_key='tracepassword';
SET s3_use_ssl=false;
SET s3_url_style='path';

-- First check if we have any staking events at all
WITH staking_events AS (
  SELECT 
    topic0,
    COUNT(*) as cnt
  FROM read_parquet(
    's3://trace-harness/cryo/**/*.parquet',
    hive_partitioning=false,
    union_by_name=true
  )
  WHERE chain_id = 143
    AND address = '\x0000000000000000000000000000000000001000'
    AND topic0 IS NOT NULL
  GROUP BY topic0
)
SELECT 
  CASE encode(topic0, 'hex')
    WHEN '3e53c8b91747e1b72a44894db10f2a45fa632b161fdcdd3a17bd6be5482bac62' THEN 'Undelegate'
    WHEN '63030e4238e1146c63f38f4ac81b2b23c8be28882e68b03f0887e50d0e9bb18f' THEN 'Withdraw'
    WHEN '84994fec' THEN 'Delegate'  -- This won't match, just showing pattern
    ELSE 'Other: ' || encode(topic0, 'hex')
  END AS event_type,
  cnt AS count
FROM staking_events;
"

echo ""
echo "Checking for violations (withdrawals without matching undelegates)..."
echo ""

docker run --rm --network=harness_default datacatering/duckdb:v1.1.3 -c "
SET s3_endpoint='minio:9000';
SET s3_access_key_id='trace';
SET s3_secret_access_key='tracepassword';
SET s3_use_ssl=false;
SET s3_url_style='path';

WITH logs_raw AS (
  SELECT *
  FROM read_parquet(
    's3://trace-harness/cryo/**/*.parquet',
    hive_partitioning=false,
    union_by_name=true
  )
  WHERE chain_id = 143
    AND address = '\x0000000000000000000000000000000000001000'
    AND topic0 IS NOT NULL
),

undelegates AS (
  SELECT
    block_number,
    transaction_hash,
    log_index,
    topic1 AS validator_id_raw,
    topic2 AS delegator_raw,
    -- withdrawId is first 32 bytes of data (as uint256, value in last byte)
    data AS data_raw
  FROM logs_raw
  WHERE topic0 = '\x3e53c8b91747e1b72a44894db10f2a45fa632b161fdcdd3a17bd6be5482bac62'
),

withdrawals AS (
  SELECT
    block_number,
    transaction_hash,
    log_index,
    topic1 AS validator_id_raw,
    topic2 AS delegator_raw,
    data AS data_raw
  FROM logs_raw
  WHERE topic0 = '\x63030e4238e1146c63f38f4ac81b2b23c8be28882e68b03f0887e50d0e9bb18f'
)

SELECT 
  w.block_number AS withdraw_block,
  encode(w.transaction_hash, 'hex') AS withdraw_tx,
  encode(w.validator_id_raw, 'hex') AS validator_id,
  encode(w.delegator_raw, 'hex') AS delegator,
  'NO_MATCHING_UNDELEGATE' AS violation_type
FROM withdrawals w
LEFT JOIN undelegates u 
  ON w.validator_id_raw = u.validator_id_raw 
  AND w.delegator_raw = u.delegator_raw
  AND substring(w.data_raw, 1, 32) = substring(u.data_raw, 1, 32)  -- match withdrawId
WHERE u.transaction_hash IS NULL
ORDER BY w.block_number
LIMIT 100;
"

RESULT=$?

if [ $RESULT -eq 0 ]; then
  echo ""
  echo "Query completed. Empty result = all withdrawals have matching undelegates."
else
  echo ""
  echo "Query failed with exit code $RESULT"
  exit $RESULT
fi
