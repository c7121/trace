# TVL Drop Detection Diagnostic

Detects if a contract's Total Value Locked (TVL) has dropped more than a specified threshold within a sliding window of blocks. Useful for monitoring DeFi protocols for potential exploits, rug pulls, or anomalous behavior.

See also: [Data Verification Runbook](../../../docs/examples/data_verification.md)

## Two Query Types

### 1. Native MON TVL (`native_mon_tvl.sql`)

Tracks native MON balance changes using `geth_balance_diffs` data from cryo.

**Requires**: `geth_balance_diffs` stream synced

### 2. ERC20 Token TVL (`erc20_token_tvl.sql`)

Tracks ERC20 token balances by monitoring `Transfer` events to/from the contract.

**Requires**: `logs` stream synced

**Transfer event signature**:
```
Transfer(address indexed from, address indexed to, uint256 value)
topic0 = 0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef
```

## Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `contract_address` | Address to monitor | (required) |
| `token_address` | ERC20 token to track (or NULL for any) | NULL |
| `window_blocks` | Sliding window size in blocks | 50000 (~5.5 hrs) |
| `drop_threshold` | Max allowed drop ratio (0.5 = 50%) | 0.5 |

## Usage

```bash
# Quick check (shows data availability)
./run.sh native 0x1234...5678
./run.sh erc20 0x1234...5678 0xTokenAddr...

# For full analysis, edit the SQL files with specific addresses:
vim native_mon_tvl.sql  # Edit SET VARIABLE lines
docker run --rm --network=harness_default datacatering/duckdb:v1.1.3 < native_mon_tvl.sql
```

## Query Logic

1. **Collect balance changes**: From balance_diffs (MON) or Transfer events (ERC20)
2. **Compute running balance**: Cumulative sum ordered by block
3. **Compare windows**: For each block, compare to balance X blocks ago
4. **Flag violations**: Where `(prev_balance - current_balance) / prev_balance > threshold`

## Limitations

- ERC20 query counts transfers, not actual amounts (would need uint256 decoding)
- Native MON query requires `geth_balance_diffs` which may not be synced
- Window comparison is approximate (finds closest block in range)

## Example Addresses to Monitor

| Protocol | Address | Type |
|----------|---------|------|
| Staking Precompile | `0x0000...1000` | Native MON |
| (Add your protocols) | | |

## Alert Thresholds

| Threshold | Meaning | Use Case |
|-----------|---------|----------|
| 0.5 (50%) | Major drain | Critical alert |
| 0.2 (20%) | Significant movement | Warning |
| 0.1 (10%) | Notable change | Monitoring |
