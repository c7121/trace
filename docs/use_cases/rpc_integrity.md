# RPC Integrity Checking

Verify RPC providers return correct data by cross-referencing multiple sources.

## Problem

RPC providers can return stale, incorrect, or manipulated data. Without verification, downstream analysis may be silently wrong.

## Solution

Run parallel queries against multiple RPC endpoints. Compare block hashes, transaction counts, and state roots. Alert on divergence.

## Implementation

- **Operator**: `rpc_integrity_check` (ecs_rust)
- **Activation**: reactive on `hot_blocks`
- **Output**: `rpc_divergence_events`
- **Alert**: trigger when providers disagree on block hash

## Example Job

```yaml
- name: rpc_integrity
  activation: reactive
  runtime: ecs_rust
  operator: rpc_integrity_check
  input_datasets: [hot_blocks]
  output_dataset: rpc_divergence_events
  secrets: [rpc_key_primary, rpc_key_secondary]
  config:
    providers: [primary, secondary]
    compare_fields: [block_hash, tx_count, state_root]
```
