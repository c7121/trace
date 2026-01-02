# Validator Monitoring

Track validator performance: missed slots, attestations, and reward rates.

## Problem

Validator operators need visibility into performance to optimize returns and diagnose issues.

## Solution

Ingest validator data per epoch. Track missed slots, attestation inclusion distance, and rewards. Alert on degradation.

## Implementation

- **Operator**: `validator_stats` (lambda)
- **Activation**: source
- **Trigger**: cron schedule (see [Concepts](../specs/dag_configuration.md#concepts))
- **Output**: `validator_performance`
- **Alert**: fire on missed slots or reward anomalies

Configure this as a source DAG job using `operator: validator_stats` with `source.kind: cron`. See [validator_stats](../architecture/operators/validator_stats.md#example-dag-config) for an example job entry, and [dag_configuration.md](../specs/dag_configuration.md) for the full job YAML schema.
