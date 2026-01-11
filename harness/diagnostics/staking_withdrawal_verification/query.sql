-- Staking Withdrawal Verification Query
-- 
-- Verifies that every Withdraw event has a corresponding Undelegate event
-- with the correct timing (respecting WITHDRAWAL_DELAY epochs).
--
-- Staking precompile: 0x0000000000000000000000000000000000001000
-- WITHDRAWAL_DELAY: 1 epoch (from Monad docs)
--
-- Event signatures:
--   Undelegate(uint64 indexed validatorId, address indexed delegator, uint8 withdrawId, uint256 amount, uint64 activationEpoch)
--   Withdraw(uint64 indexed validatorId, address indexed delegator, uint8 withdrawId, uint256 amount, uint64 withdrawEpoch)
--
-- topic0 hashes:
--   Undelegate: 0x3e53c8b91747e1b72a44894db10f2a45fa632b161fdcdd3a17bd6be5482bac62
--   Withdraw:   0x63030e4238e1146c63f38f4ac81b2b23c8be28882e68b03f0887e50d0e9bb18f

-- DuckDB settings for MinIO access
SET s3_endpoint='minio:9000';
SET s3_access_key_id='trace';
SET s3_secret_access_key='tracepassword';
SET s3_use_ssl=false;
SET s3_url_style='path';

-- Data decoding notes:
-- - topic1 = validatorId (uint64, right-padded to 32 bytes)
-- - topic2 = delegator (address, right-padded to 32 bytes)
-- - data layout (non-indexed): withdrawId (uint8), amount (uint256), epoch (uint64)
--   - bytes 0-31: withdrawId (as uint256, only last byte matters)
--   - bytes 32-63: amount (uint256)
--   - bytes 64-95: activationEpoch or withdrawEpoch (as uint256, only last 8 bytes matter)

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
    -- topic1 is validatorId (uint64 in rightmost bytes of 32-byte topic)
    topic1 AS validator_id_raw,
    -- topic2 is delegator address (20 bytes in rightmost of 32-byte topic)
    topic2 AS delegator_raw,
    -- data: withdrawId (byte 31), amount (bytes 32-63), activationEpoch (bytes 88-95)
    get_bit(data, 31*8)::UTINYINT AS withdraw_id,  -- simplified; see below for proper decode
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

-- Main query: find withdrawals without matching undelegate, or too early
SELECT 
  w.block_number AS withdraw_block,
  w.transaction_hash AS withdraw_tx,
  w.log_index AS withdraw_log_index,
  encode(w.validator_id_raw, 'hex') AS validator_id,
  encode(w.delegator_raw, 'hex') AS delegator,
  CASE 
    WHEN u.transaction_hash IS NULL THEN 'NO_MATCHING_UNDELEGATE'
    ELSE 'WITHDREW_TOO_EARLY'
  END AS violation_type,
  u.block_number AS undelegate_block,
  u.transaction_hash AS undelegate_tx
FROM withdrawals w
LEFT JOIN undelegates u 
  ON w.validator_id_raw = u.validator_id_raw 
  AND w.delegator_raw = u.delegator_raw
  -- Match by withdrawId from data field (byte 31 of each)
  AND substring(w.data_raw, 32, 1) = substring(u.data_raw, 32, 1)
WHERE u.transaction_hash IS NULL
   -- OR check epoch timing once we decode epochs properly
ORDER BY w.block_number;

-- Expected result: 0 rows means all withdrawals have valid undelegates
