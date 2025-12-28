# Alerting

User-defined alerts on blockchain data — define conditions, evaluate against live and historical data, deliver notifications.

## Overview

Alerting is a three-stage pipeline:

1. **Definition** — User creates alert rule with conditions and channels
2. **Evaluation** — System evaluates conditions against data (real-time or historical)
3. **Delivery** — System sends notifications to configured channels

## Alert Definitions

Stored in `alert_definitions` table:

```sql
CREATE TABLE alert_definitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    condition JSONB NOT NULL,         -- UDF or expression (see below)
    channels JSONB NOT NULL,          -- email, sms, webhook configs
    visibility TEXT NOT NULL DEFAULT 'private',
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);
```

## Condition Model (UDF)

Users write alert conditions as UDFs. See [udf.md](udf.md) for runtimes, sandbox, resource limits, and determinism requirements.

## Evaluation

Three `alert_evaluate` operators — one per runtime:

- `alert_evaluate_ts` (Lambda)
- `alert_evaluate_py` (ECS Python)
- `alert_evaluate_rs` (ECS Rust)

All share the same contract:
- **Input**: `hot_blocks` (or other watched dataset) + `alert_definitions`
- **Output**: `triggered_alerts`
- **Execution**: `PerUpdate` — evaluates each new block/event

### Incremental Processing

```yaml
- name: alert_evaluate
  incremental:
    mode: cursor
    cursor_column: block_number
    unique_key: [alert_definition_id, block_hash, tx_hash]
  update_strategy: append
```

## Delivery

`alert_deliver` operator (Lambda):
- Reads from `triggered_alerts`
- Delivers to configured channels
- Records delivery status

### Channels

| Channel | Provider | Config |
|---------|----------|--------|
| Email | SES (VPC endpoint) | `to`, `subject_template` |
| SMS | SNS (VPC endpoint) | `phone_number` |
| Webhook | HTTP (allowlisted URLs) | `url`, `headers`, `method` |
| Slack | Slack API (allowlisted) | `webhook_url`, `channel` |

## Deduplication

Alerts must not re-fire on reprocessing. Dedupe key:

```sql
CREATE TABLE alert_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_definition_id UUID NOT NULL,
    block_hash TEXT NOT NULL,         -- changes on reorg
    tx_hash TEXT,                     -- nullable for block-level alerts
    block_number BIGINT NOT NULL,     -- for display, not dedupe
    triggered_at TIMESTAMPTZ DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    delivery_status TEXT,
    UNIQUE (alert_definition_id, block_hash, tx_hash)
);
```

### Behavior Matrix

| Scenario | Result |
|----------|--------|
| Normal processing | Alert created, delivered |
| Reprocess same block | Dedupe → no new alert |
| Reorg, same tx in new block | New alert (different `block_hash`) |
| Reorg, tx dropped | No alert (tx not in canonical chain) |

### Append-Only

`alert_events` is append-only:
- Never delete (audit trail)
- Never update (immutable record)
- Orphaned alerts remain, can be flagged via join

## Rate Limiting

Per-channel throttling prevents spam. See [backlog](../plan/backlog.md#alerting).

## DAG Configuration

```yaml
# Evaluate alerts on new blocks
- name: alert_evaluate
  activation: reactive
  runtime: ecs_rust      # or lambda, ecs_python
  operator: alert_evaluate_rs
  execution_strategy: PerUpdate
  idle_timeout: 5m
  input_datasets: [hot_blocks, alert_definitions]
  output_dataset: triggered_alerts
  timeout_seconds: 60

# Deliver triggered alerts
- name: alert_deliver
  activation: reactive
  runtime: lambda
  operator: alert_deliver
  execution_strategy: PerUpdate
  idle_timeout: 5m
  input_datasets: [triggered_alerts]
  output_dataset: alert_deliveries
  timeout_seconds: 60
```

## Related

- [alert_evaluate_ts](../architecture/operators/alert_evaluate_ts.md)
- [alert_evaluate_py](../architecture/operators/alert_evaluate_py.md)
- [alert_evaluate_rs](../architecture/operators/alert_evaluate_rs.md)
- [alert_deliver](../architecture/operators/alert_deliver.md)
- [data_versioning.md](../architecture/data_versioning.md) — deduplication details
