# Validator monitoring

Track validator performance: missed slots, attestations, and reward rates.

Operator: [`validator_stats`](../specs/operators/validator_stats.md)

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

Configure this as a source DAG job using `operator: validator_stats` with `source.kind: cron`. See [Example DAG Config](../specs/operators/validator_stats.md#example-dag-config) for an example job entry, and [dag_configuration.md](../specs/dag_configuration.md) for the full job YAML schema.

