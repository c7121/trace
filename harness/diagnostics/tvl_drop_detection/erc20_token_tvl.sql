-- TVL Drop Detection Query (ERC20 Token)
--
-- Detects if a contract's ERC20 token balance has dropped more than a threshold
-- within a sliding window of X blocks.
--
-- Uses logs dataset from cryo sync, tracking Transfer events.
--
-- Parameters to customize:
--   CONTRACT_ADDRESS: The address holding the tokens (as hex blob)
--   TOKEN_ADDRESS: The ERC20 token contract (as hex blob), or NULL for any token
--   WINDOW_BLOCKS: Number of blocks for the sliding window
--   DROP_THRESHOLD: Maximum allowed drop (0.5 = 50%)
--
-- ERC20 Transfer event:
--   Transfer(address indexed from, address indexed to, uint256 value)
--   topic0 = 0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef
--   topic1 = from (padded to 32 bytes)
--   topic2 = to (padded to 32 bytes)
--   data = value (uint256)

SET s3_endpoint='minio:9000';
SET s3_access_key_id='trace';
SET s3_secret_access_key='tracepassword';
SET s3_use_ssl=false;
SET s3_url_style='path';

-- Configuration (edit these)
-- Example: monitoring a hypothetical vault contract for USDC-like token
SET VARIABLE contract_address = '\x0000000000000000000000000000000000000000';  -- REPLACE with actual address
SET VARIABLE token_address = NULL;  -- NULL = any token, or set specific token address
SET VARIABLE window_blocks = 50000;  -- ~5.5 hours
SET VARIABLE drop_threshold = 0.5;   -- 50% drop

WITH transfer_events AS (
  SELECT
    block_number,
    transaction_hash,
    -- address is the token contract
    address AS token,
    -- topic1 = from, topic2 = to (right-aligned in 32 bytes, take last 20)
    substring(topic1, 13, 20) AS from_addr,
    substring(topic2, 13, 20) AS to_addr,
    -- data = transfer amount as uint256
    data AS amount_raw
  FROM read_parquet(
    's3://trace-harness/cryo/**/*.parquet',
    hive_partitioning=false,
    union_by_name=true
  )
  WHERE chain_id = 143
    AND topic0 = '\xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef'
    AND (getvariable('token_address') IS NULL OR address = getvariable('token_address'))
),

-- Compute balance changes for the monitored contract
balance_changes AS (
  SELECT
    block_number,
    token,
    CASE
      -- Inflow: contract receives tokens
      WHEN to_addr = substring(getvariable('contract_address'), 1, 20) THEN 1
      -- Outflow: contract sends tokens  
      WHEN from_addr = substring(getvariable('contract_address'), 1, 20) THEN -1
      ELSE 0
    END AS direction,
    amount_raw
  FROM transfer_events
  WHERE to_addr = substring(getvariable('contract_address'), 1, 20)
     OR from_addr = substring(getvariable('contract_address'), 1, 20)
),

-- Aggregate by block and compute running balance per token
block_balances AS (
  SELECT
    block_number,
    token,
    SUM(direction) AS net_flow_count  -- simplified; actual would decode amount
  FROM balance_changes
  GROUP BY block_number, token
),

running_balance AS (
  SELECT
    block_number,
    token,
    SUM(net_flow_count) OVER (PARTITION BY token ORDER BY block_number) AS cumulative_flows
  FROM block_balances
),

-- Compare current vs X blocks ago
balance_comparison AS (
  SELECT
    curr.block_number AS current_block,
    curr.token,
    curr.cumulative_flows AS current_flows,
    prev.block_number AS prev_block,
    prev.cumulative_flows AS prev_flows
  FROM running_balance curr
  JOIN running_balance prev 
    ON curr.token = prev.token
    AND prev.block_number BETWEEN curr.block_number - getvariable('window_blocks') - 1000 
                              AND curr.block_number - getvariable('window_blocks')
)

-- Find large drops
SELECT
  current_block,
  encode(token, 'hex') AS token_address,
  prev_block,
  current_block - prev_block AS blocks_elapsed,
  prev_flows AS prev_transfer_count,
  current_flows AS current_transfer_count,
  CASE 
    WHEN prev_flows > 0 
    THEN ROUND((prev_flows - current_flows)::FLOAT / prev_flows * 100, 2)
    ELSE 0
  END AS drop_percent
FROM balance_comparison
WHERE prev_flows > current_flows
  AND (prev_flows - current_flows)::FLOAT / NULLIF(prev_flows, 0) > getvariable('drop_threshold')
ORDER BY current_block;

-- Note: This simplified version counts transfer events, not actual token amounts.
-- For precise TVL tracking, you'd need to decode the uint256 amount from data field.
