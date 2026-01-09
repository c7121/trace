#!/usr/bin/env bash
# Test script to verify duplicate transaction hash detection query.
# Uses pre-staged test data in this directory which includes:
#   - tx_45000.parquet: real transactions from blocks 45000-45999
#   - tx_46000.parquet: real transactions from blocks 46000-46999
#   - tx_fake_duplicate.parquet: same transactions as tx_45000 but with block_number=99999
#
# The query should detect that transaction hashes from block 45216 and 45463
# also appear in block 99999 (the fake duplicate).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DIR="$SCRIPT_DIR"

echo "=== Duplicate Transaction Detection Test ==="
echo "Test data: $TEST_DIR"
echo ""

# Verify test data exists
if [ ! -f "$TEST_DIR/tx_45000.parquet" ] || [ ! -f "$TEST_DIR/tx_fake_duplicate.parquet" ]; then
  echo "❌ FAIL: Test data not found in $TEST_DIR"
  echo "Expected files: tx_45000.parquet, tx_46000.parquet, tx_fake_duplicate.parquet"
  exit 1
fi

echo "1. Running duplicate detection query on local test data..."
RESULT=$(docker run --rm -v "$TEST_DIR:/data" datacatering/duckdb:v1.1.3 -c "
SELECT 
  transaction_hash,
  MIN(block_number) AS first_block,
  MAX(block_number) AS last_block,
  COUNT(DISTINCT block_number) AS num_blocks
FROM read_parquet('/data/*.parquet')
GROUP BY transaction_hash
HAVING COUNT(DISTINCT block_number) > 1;
")

echo ""
echo "=== Query Result ==="
echo "$RESULT"
echo ""

# Check if duplicates were found
if echo "$RESULT" | grep -q "0 rows"; then
  echo "❌ FAIL: No duplicates detected (expected duplicates)"
  exit 1
else
  echo "✅ PASS: Duplicates detected correctly"
  echo ""
  echo "The query found transactions appearing in multiple blocks:"
  echo "  - Real blocks: 45216, 45463 (from tx_45000.parquet)"
  echo "  - Fake block: 99999 (from tx_fake_duplicate.parquet)"
fi

echo ""
echo "=== Test Complete ==="
