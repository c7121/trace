# RPC Integrity Checking

Verify RPC providers return correct data by cross-referencing multiple sources.

## Problem

RPC providers can return stale, incorrect, or manipulated data. Without verification, downstream analysis may be silently wrong.

## Solution

Run parallel queries against multiple RPC endpoints. Compare block hashes, transaction counts, and state roots. Alert on divergence.

## Implementation

- **Operator**: `rpc_integrity_check` (lambda)
- **Activation**: reactive
- **Trigger**: dataset events on `hot_blocks` (see [Concepts](../capabilities/dag_configuration.md#concepts))
- **Output**: `rpc_divergence_events`
- **Alert**: fire when providers disagree on block hash

Configure this as a reactive DAG job using `operator: rpc_integrity_check`. See [rpc_integrity_check](../architecture/operators/rpc_integrity_check.md#example-dag-config) for an example job entry, and [dag_configuration.md](../capabilities/dag_configuration.md) for the full job YAML schema.
