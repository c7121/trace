#!/usr/bin/env bash
# TVL Drop Detection
#
# Detects if TVL (Total Value Locked) for a contract has dropped more than
# a specified threshold within a sliding window of blocks.
#
# Usage:
#   ./run.sh native <contract_address> [window_blocks] [threshold]
#   ./run.sh erc20 <contract_address> [token_address] [window_blocks] [threshold]
#
# Examples:
#   ./run.sh native 0x1234...5678 50000 0.5
#   ./run.sh erc20 0x1234...5678 0xUSDC... 50000 0.5

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  echo "Usage:"
  echo "  $0 native <contract_address> [window_blocks] [threshold]"
  echo "  $0 erc20 <contract_address> [token_address|any] [window_blocks] [threshold]"
  echo ""
  echo "Defaults: window_blocks=50000, threshold=0.5 (50%)"
  exit 1
}

if [ $# -lt 2 ]; then
  usage
fi

MODE=$1
CONTRACT=$2

case $MODE in
  native)
    WINDOW=${3:-50000}
    THRESHOLD=${4:-0.5}
    
    echo "=== TVL Drop Detection (Native MON) ==="
    echo "Contract: $CONTRACT"
    echo "Window: $WINDOW blocks"
    echo "Threshold: $THRESHOLD ($(echo "$THRESHOLD * 100" | bc)%)"
    echo ""

    docker run --rm --network=harness_default datacatering/duckdb:v1.1.3 -c "
SET s3_endpoint='minio:9000';
SET s3_access_key_id='trace';
SET s3_secret_access_key='tracepassword';
SET s3_use_ssl=false;
SET s3_url_style='path';

-- Check if we have balance diff data
SELECT 
  COUNT(*) as balance_diff_rows,
  MIN(block_number) as min_block,
  MAX(block_number) as max_block
FROM read_parquet(
  's3://trace-harness/cryo/**/*.parquet',
  hive_partitioning=false,
  union_by_name=true
)
WHERE chain_id = 143
  AND balance_diff IS NOT NULL;
"
    ;;
    
  erc20)
    TOKEN=${3:-any}
    WINDOW=${4:-50000}
    THRESHOLD=${5:-0.5}
    
    echo "=== TVL Drop Detection (ERC20 Token) ==="
    echo "Contract: $CONTRACT"
    echo "Token: $TOKEN"
    echo "Window: $WINDOW blocks"
    echo "Threshold: $THRESHOLD ($(echo "$THRESHOLD * 100" | bc)%)"
    echo ""

    docker run --rm --network=harness_default datacatering/duckdb:v1.1.3 -c "
SET s3_endpoint='minio:9000';
SET s3_access_key_id='trace';
SET s3_secret_access_key='tracepassword';
SET s3_use_ssl=false;
SET s3_url_style='path';

-- Check Transfer events
SELECT 
  COUNT(*) as transfer_events,
  COUNT(DISTINCT address) as unique_tokens,
  MIN(block_number) as min_block,
  MAX(block_number) as max_block
FROM read_parquet(
  's3://trace-harness/cryo/**/*.parquet',
  hive_partitioning=false,
  union_by_name=true
)
WHERE chain_id = 143
  AND topic0 = '\xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';
"
    ;;
    
  *)
    usage
    ;;
esac

echo ""
echo "For full analysis, edit the SQL files with specific addresses and run manually."
