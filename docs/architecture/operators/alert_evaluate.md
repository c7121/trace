# alert_evaluate

Evaluate alert conditions against Trace data and emit `alert_events`.

This operator executes **untrusted user-supplied code** (a UDF bundle) and is intentionally constrained:
- reads only via Query Service (capability token)
- emits events only via task-scoped APIs (`/v1/task/buffer-publish` using the same capability token + lease fencing)
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
3. Write an **alert event batch artifact** (object storage) and publish it via `POST /v1/task/buffer-publish` to the buffered dataset `alert_events`.

The batch artifact format and the required alert event schema are defined in `docs/specs/alerting.md`.

## Outputs

- Buffered dataset: `alert_events` (append; row-level idempotency required)

## Reliability + Idempotency

- Alert evaluation runs under **at-least-once** execution. Retries and duplicates are expected.
- Each emitted row MUST include a deterministic `dedupe_key` that is stable across retries/reorg replays.
- The sink enforces idempotency with `UNIQUE(org_id, dedupe_key)`.
- The buffer sink MUST reject malformed batches (DLQ); this prevents silent corruption.

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
    max_lookback_seconds: 3600

  # UDF bundle reference
  udf:
    bundle_id: "<bundle-id>"
    entrypoint: "trace.handler"
```

## Notes

- The `udf` block is required for user-defined evaluation logic. Built-in evaluation engines are intentionally out of scope for v1.
- Untrusted code must never call `/internal/*` endpoints.
