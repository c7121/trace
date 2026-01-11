# Data verification

This page is a link-first index of runnable diagnostics for verifying data integrity in Trace Lite.

## Start here

- Bring up the Trace Lite local stack and sync some data: [lite_local_cryo_sync.md](lite_local_cryo_sync.md)
- Run diagnostics from `harness/diagnostics/*`. The runnable scripts and READMEs are the canonical home for SQL, DuckDB commands, and object-store paths.

## Runnable diagnostics

### Duplicate transaction detection

Detect transaction hashes that appear in multiple blocks.

- Diagnostic: [harness/diagnostics/duplicate_tx_detection/](../../harness/diagnostics/duplicate_tx_detection/)
- Run fixture test: `cd harness/diagnostics/duplicate_tx_detection && ./run_test.sh`

### Staking withdrawal verification

Verify that every Monad staking `Withdraw` has a corresponding prior `Undelegate`.

- Diagnostic: [harness/diagnostics/staking_withdrawal_verification/](../../harness/diagnostics/staking_withdrawal_verification/)
- Run: `cd harness/diagnostics/staking_withdrawal_verification && ./run.sh`
- Requires: logs data synced

### TVL drop detection

Detect large TVL drops over a sliding window of blocks.

- Diagnostic: [harness/diagnostics/tvl_drop_detection/](../../harness/diagnostics/tvl_drop_detection/)
- Run:
  - `cd harness/diagnostics/tvl_drop_detection && ./run.sh native <contract_address>`
  - `cd harness/diagnostics/tvl_drop_detection && ./run.sh erc20 <contract_address> <token_address|any>`
- Requires:
  - native: `geth_balance_diffs` data synced
  - erc20: logs data synced

## Notes

- Local harness diagnostics assume MinIO inside the `harness_default` docker network and the default bucket `trace-harness`. Prefer running the scripts instead of copying commands into other docs.

## Adding new diagnostics

- Add a folder under `harness/diagnostics/` with a `README.md` and a runnable script (`run.sh` or `run_test.sh`).
- Keep the diagnostic SQL in the same folder as the script to avoid drift.
