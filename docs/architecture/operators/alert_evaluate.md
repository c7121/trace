# alert_evaluate

Evaluate alert conditions against Trace data and emit `alert_events`.

This operator executes **untrusted user-supplied code** (a UDF bundle) and is intentionally constrained:
- reads only via Query Service (capability token)
- writes only via task-scoped APIs (worker token)
- no third-party network egress; outbound notifications are handled by Delivery Service via `alert_route`

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `lambda` (v1) |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate |
| **Idle Timeout** | `5m` |

## Inputs

Alert evaluation is usually triggered by upstream dataset updates (e.g., new blocks, new decoded events) or by scheduled re-evaluation.

The task payload MUST include enough information to evaluate without direct database credentials:
- alert definition (or a reference that Query Service can resolve under the capability token)
- evaluation window/context (e.g., event time bounds, partition keys, cursor positions)
- any pinned dataset versions the evaluation is allowed to read

## Behavior

1. Load evaluation inputs using Query Service (`/v1/task/query`) authenticated by the **capability token**.
2. Execute the user bundle’s evaluation logic over the inputs.
3. For each triggered condition, emit an alert event row into the buffered dataset via `POST /v1/task/buffer-publish`.

## Outputs

- Buffered dataset: `alert_events` (append; row-level idempotency required)

## Reliability + Idempotency

- Alert evaluation runs under **at-least-once** execution. Retries and duplicates are expected.
- Each emitted row MUST include a deterministic `dedupe_key` that is stable across retries/reorg replays.
- The sink enforces idempotency with `UNIQUE(dedupe_key)` (or equivalent) and upserts.

## Example DAG config

```yaml
- name: alert_evaluate
  activation: reactive
  runtime: lambda
  operator: alert_evaluate
  execution_strategy: PerUpdate
  idle_timeout: 5m
  inputs:
    - from: { job: block_follower, output: 0 }
  outputs: 1
  update_strategy: append
  unique_key: [dedupe_key]
  config:
    # operator-specific config for selecting which alerts to evaluate (optional)
    max_lookback_seconds: 3600

  # UDF bundle reference (see `docs/specs/udf.md`)
  udf:
    bundle_id: "<bundle-id>"
    entrypoint: "trace.handler"
```

## Notes

- The `udf` block is required for user-defined evaluation logic. Built-in evaluation engines are intentionally out of scope for v1.
- See `docs/specs/alerting.md` for the alerting feature design and `docs/architecture/contracts.md` for task-scoped API contracts.
