-- TVL Drop Detection Query (Native MON)
--
-- Detects if a contract's native MON balance has dropped more than a threshold
-- within a sliding window of X blocks.
--
-- Uses geth_balance_diffs dataset from cryo sync.
--
-- Parameters to customize:
--   CONTRACT_ADDRESS: The address to monitor (as hex blob)
--   WINDOW_BLOCKS: Number of blocks for the sliding window
--   DROP_THRESHOLD: Maximum allowed drop (0.5 = 50%)

SET s3_endpoint='minio:9000';
SET s3_access_key_id='trace';
SET s3_secret_access_key='tracepassword';
SET s3_use_ssl=false;
SET s3_url_style='path';

-- Configuration (edit these)
SET VARIABLE contract_address = '\x0000000000000000000000000000000000001000';  -- staking precompile as example
SET VARIABLE window_blocks = 50000;  -- ~5.5 hours at 1 block/400ms
SET VARIABLE drop_threshold = 0.5;   -- 50% drop

WITH balance_diffs AS (
  SELECT
    block_number,
    address,
    -- balance_diff can be positive (deposit) or negative (withdrawal)
    -- Cryo stores this as balance change per block
    balance_diff
  FROM read_parquet(
    's3://trace-harness/cryo/**/*.parquet',
    hive_partitioning=false,
    union_by_name=true
  )
  WHERE chain_id = 143
    AND address = getvariable('contract_address')
    AND balance_diff IS NOT NULL
),

-- Compute running balance at each block with activity
running_balance AS (
  SELECT
    block_number,
    SUM(balance_diff) OVER (ORDER BY block_number) AS cumulative_balance
  FROM balance_diffs
),

-- Self-join to compare current balance vs balance X blocks ago
balance_comparison AS (
  SELECT
    curr.block_number AS current_block,
    curr.cumulative_balance AS current_balance,
    prev.block_number AS prev_block,
    prev.cumulative_balance AS prev_balance,
    CASE 
      WHEN prev.cumulative_balance > 0 
      THEN (prev.cumulative_balance - curr.cumulative_balance) / prev.cumulative_balance
      ELSE 0
    END AS drop_ratio
  FROM running_balance curr
  JOIN running_balance prev 
    ON prev.block_number <= curr.block_number - getvariable('window_blocks')
    AND prev.block_number > curr.block_number - getvariable('window_blocks') - 1000  -- limit lookback
)

-- Find violations where drop exceeds threshold
SELECT
  current_block,
  prev_block,
  current_block - prev_block AS blocks_elapsed,
  prev_balance,
  current_balance,
  ROUND(drop_ratio * 100, 2) AS drop_percent
FROM balance_comparison
WHERE drop_ratio > getvariable('drop_threshold')
ORDER BY current_block;

-- Expected: 0 rows means no suspicious drops
