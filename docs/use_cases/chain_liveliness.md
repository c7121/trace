# Chain Liveliness Monitoring

Detect when a blockchain stops producing blocks.

## Problem

Chains can halt due to consensus failures, network partitions, or bugs. Early detection enables rapid response.

## Solution

Track time since last block. Alert when gap exceeds threshold for the chain's expected block time.

## Implementation

- **Operator**: `liveliness_monitor` (ecs_rust)
- **Activation**: source (always_on)
- **Output**: `liveliness_events`
- **Alert**: trigger when block gap exceeds threshold

## Example Job

```yaml
- name: chain_liveliness
  activation: source
  runtime: ecs_rust
  operator: liveliness_monitor
  source:
    kind: always_on
  output_dataset: liveliness_events
  secrets: [monad_rpc_key]
  config:
    chain_id: 10143
    expected_block_time_ms: 500
    alert_threshold_ms: 5000
```
