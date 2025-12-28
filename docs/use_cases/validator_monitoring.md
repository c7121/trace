# Validator Monitoring

Track validator performance: missed slots, attestations, and reward rates.

## Problem

Validator operators need visibility into performance to optimize returns and diagnose issues.

## Solution

Ingest validator data per epoch. Track missed slots, attestation inclusion distance, and rewards. Alert on degradation.

## Implementation

- **Operator**: `validator_stats` (ecs_rust)
- **Activation**: reactive on block/epoch boundaries
- **Output**: `validator_performance`
- **Alert**: trigger on missed slots or reward anomalies

## Example Job

```yaml
- name: validator_stats
  activation: reactive
  runtime: ecs_rust
  operator: validator_stats
  input_datasets: [hot_blocks]
  output_dataset: validator_performance
  secrets: [beacon_api_key]
  config:
    validator_indices: [12345, 12346, 12347]
    track_metrics: [missed_slots, attestation_distance, rewards]
```
