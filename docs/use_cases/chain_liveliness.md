# Chain Liveliness Monitoring

Detect when a blockchain stops producing blocks.

## Problem

Chains can halt due to consensus failures, network partitions, or bugs. Early detection enables rapid response.

## Solution

Track time since last block. Alert when gap exceeds threshold for the chain's expected block time.

## Implementation

- **Operator**: `liveliness_monitor` (lambda)
- **Activation**: source
- **Trigger**: cron schedule (see [Concepts](../features/dag_configuration.md#concepts))
- **Output**: `liveliness_events`
- **Alert**: fire when block gap exceeds threshold

Configure this as a source DAG job using `operator: liveliness_monitor` with `source.kind: cron`. See [liveliness_monitor](../architecture/operators/liveliness_monitor.md#example-dag-config) for an example job entry, and [dag_configuration.md](../features/dag_configuration.md) for the full job YAML schema.
