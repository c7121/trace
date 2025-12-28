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

Configure this as a reactive DAG job using `operator: validator_stats`. See [dag_configuration.md](../capabilities/dag_configuration.md) for the job YAML schema.
