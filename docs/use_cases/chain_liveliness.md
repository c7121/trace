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

Configure this as a source DAG job using `operator: liveliness_monitor` with `source.kind: always_on`. See [dag_configuration.md](../capabilities/dag_configuration.md) for the job YAML schema.
