# Staking Withdrawal Verification Diagnostic

Verifies that every `Withdraw` event from the Monad staking precompile has a corresponding `Undelegate` event that was called beforehand.

See also: [Data Verification Runbook](../../../docs/runbooks/data_verification.md)

## Background

The Monad staking precompile (`0x0000000000000000000000000000000000001000`) requires a two-step withdrawal process:

1. **Undelegate**: User calls `undelegate(validatorId, amount, withdrawId)` to initiate withdrawal
2. **Wait**: Must wait `WITHDRAWAL_DELAY` (1 epoch) before funds are claimable
3. **Withdraw**: User calls `withdraw(validatorId, withdrawId)` to claim funds

Each step emits an event:
- `Undelegate(uint64 indexed validatorId, address indexed delegator, uint8 withdrawId, uint256 amount, uint64 activationEpoch)`
- `Withdraw(uint64 indexed validatorId, address indexed delegator, uint8 withdrawId, uint256 amount, uint64 withdrawEpoch)`

## Invariant

For every `Withdraw` event, there MUST be a prior `Undelegate` event with:
- Same `validatorId`
- Same `delegator`
- Same `withdrawId`
- `Withdraw.withdrawEpoch >= Undelegate.activationEpoch + WITHDRAWAL_DELAY`

## Event Signatures

```
Undelegate: keccak256("Undelegate(uint64,address,uint8,uint256,uint64)")
          = 0x3e53c8b91747e1b72a44894db10f2a45fa632b161fdcdd3a17bd6be5482bac62

Withdraw:   keccak256("Withdraw(uint64,address,uint8,uint256,uint64)")
          = 0x63030e4238e1146c63f38f4ac81b2b23c8be28882e68b03f0887e50d0e9bb18f
```

## Usage

```bash
# Requires harness running with logs data synced
cd harness/diagnostics/staking_withdrawal_verification
./run.sh
```

## Files

- `query.sql` - Full annotated DuckDB query
- `run.sh` - Executable wrapper script

## Notes

- Staking was enabled after Monad mainnet genesis, so early blocks have no staking activity
- The query requires **logs** data (not just blocks/transactions) to be synced
- Expected result: 0 violations (empty result set)
