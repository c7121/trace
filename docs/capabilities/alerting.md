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
    visibility TEXT NOT NULL DEFAULT 'private',  -- see pii.md
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);
```

PII column: `alert_definitions.channels` (may include email/phone/webhook URLs). Mark it as PII in dataset metadata; see [pii.md](pii.md) for visibility and audit rules.

## Condition Model (UDF)

Users write alert conditions as UDFs. See [udf.md](udf.md) for runtimes, sandbox, resource limits, and determinism requirements.

## Alert Events (Sink)

Triggered alerts are durable facts recorded as append-only rows in `alert_events`.

Multiple jobs/operators may write to this dataset (multi-writer sink). See [ADR 0004](../architecture/adr/0004-alert-event-sinks.md).

In v1, `alert_events` is typically configured as a **buffered Postgres dataset** (SQS buffer → sink → Postgres). Producers publish records; the platform sink writes the table and emits the upstream dataset event after commit. See [ADR 0006](../architecture/adr/0006-buffered-postgres-datasets.md).

```sql
CREATE TABLE alert_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    alert_definition_id UUID REFERENCES alert_definitions(id), -- nullable for non-UDF/system alerts
    producer_job_id UUID REFERENCES jobs(id),
    producer_task_id UUID REFERENCES tasks(id),
    severity TEXT,                      -- e.g., 'info'|'warning'|'critical'
    chain_id BIGINT,
    block_number BIGINT,
    block_hash TEXT,                    -- changes on reorg
    tx_hash TEXT,                       -- nullable for block-level alerts
    source_dataset TEXT,                -- dataset that triggered the alert (optional)
    partition_key TEXT,                 -- e.g., '1000000-1010000' (optional)
    cursor_value TEXT,                  -- e.g., block height cursor (optional)
    payload JSONB NOT NULL DEFAULT '{}',-- producer-defined details
    dedupe_key TEXT NOT NULL,           -- deterministic idempotency key
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (org_id, dedupe_key)
);
```

**DAG contract:**
- Producers write `output_datasets: [alert_events]` with `update_strategy: append` and `unique_key: [dedupe_key]`.
- `dedupe_key` must be deterministic from input data and config (examples: `{alert_definition_id}:{block_hash}:{tx_hash}` or `{producer_job_id}:{chain_id}:{block_number}`).

## Evaluation (Reference Producers)

Three reference `alert_evaluate` operators evaluate `alert_definitions` and write to `alert_events`:

- `alert_evaluate_ts` (Lambda)
- `alert_evaluate_py` (ECS Python)
- `alert_evaluate_rs` (ECS Rust)

Runtime selection is per-job in the DAG. A single DAG can include multiple evaluation jobs with different runtimes.

## Delivery

`alert_deliver` operator (Lambda):
- Reads from `alert_events`
- Delivers to configured channels
- Records delivery status

Delivery outcomes are recorded in `alert_deliveries` with one row per `(alert_event_id, channel)`. Retries overwrite/update the same row (replace/upsert semantics).

```sql
CREATE TABLE alert_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    alert_event_id UUID NOT NULL REFERENCES alert_events(id),
    channel TEXT NOT NULL,              -- 'email'|'sms'|'webhook'|'slack'|'pagerduty'
    status TEXT NOT NULL,               -- 'pending'|'sending'|'delivered'|'retrying'|'failed'|...
    attempt INT NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    leased_until TIMESTAMPTZ,           -- lease for crash-safe claiming
    lease_owner TEXT,                   -- worker identity (optional)
    last_attempt_at TIMESTAMPTZ,
    provider_message_id TEXT,
    error_message TEXT,
    delivered_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (org_id, alert_event_id, channel)
);

CREATE INDEX idx_alert_deliveries_event ON alert_deliveries(alert_event_id);
CREATE INDEX idx_alert_deliveries_ready ON alert_deliveries(status, next_attempt_at);
```

PII note: delivery destinations (email addresses, phone numbers, webhook URLs) live in `alert_definitions.channels`. `alert_deliveries` should not duplicate destinations; store only channel type and provider response metadata.

### Delivery Guarantees (Never Missed, No Double-Fire)

`alert_deliveries` is the durable work queue. The delivery worker:

1. Materializes work idempotently: create one row per `(org_id, alert_event_id, channel)` (unique constraint prevents duplicates).
2. Claims work with a short lease (`leased_until`) so only one worker attempts a delivery at a time; if a worker dies mid-send, the lease expires and another worker retries.
3. Retries until success: failures set `status='retrying'` and schedule `next_attempt_at` with backoff.

To prevent double-firing external notifications, every outbound send includes a stable **idempotency key**:

- Use `alert_deliveries.id` as the idempotency key for the `(alert_event_id, channel)` delivery.
- PagerDuty: map it to `dedup_key`.
- Webhooks: send `Idempotency-Key: <delivery_id>` and include `delivery_id` in the body for receivers to dedupe.

Without downstream idempotency (provider- or receiver-side), no system can guarantee both “never missed” and “never twice” under timeouts/crashes. In that case Trace provides **at-least-once** delivery semantics.

### Channels

| Channel | Provider | Config |
|---------|----------|--------|
| Email | SES (VPC endpoint) | `to`, `subject_template` |
| SMS | SNS (VPC endpoint) | `phone_number` |
| Webhook | HTTP (allowlisted URLs) | `url`, `headers`, `method` |
| Slack | Slack API (allowlisted) | `webhook_url`, `channel` |
| PagerDuty | PagerDuty Events API (allowlisted) | `routing_key`, `dedup_key` |

### Routing (Filters)

To route alerts by severity (or any column), run multiple delivery jobs with filtered inputs. Filters are read-time predicates on the input edge; see [ADR 0007](../architecture/adr/0007-input-edge-filters.md).

```yaml
- name: deliver_critical
  operator: alert_deliver
  input_datasets:
    - name: alert_events
      where: "severity = 'critical'"
  config:
    channels: [pagerduty]

- name: deliver_low
  operator: alert_deliver
  input_datasets:
    - name: alert_events
      where: "severity IN ('info','warning')"
  config:
    channels: [slack]
```

## Deduplication

Alerts must not re-fire on reprocessing. Dedupe key:

```sql
INSERT INTO alert_events (org_id, dedupe_key, alert_definition_id, payload)
VALUES ($1, $2, $3, $4)
ON CONFLICT (org_id, dedupe_key) DO NOTHING;
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
- Never update (immutable facts)
- Orphaned alerts remain, can be flagged via join

## Rate Limiting

Per-channel throttling prevents spam. See [backlog](../plan/backlog.md#alerting).

## DAG Configuration

See operator docs for example DAG job entries:

- [alert_evaluate_ts](../architecture/operators/alert_evaluate_ts.md#example-dag-config)
- [alert_evaluate_py](../architecture/operators/alert_evaluate_py.md#example-dag-config)
- [alert_evaluate_rs](../architecture/operators/alert_evaluate_rs.md#example-dag-config)
- [alert_deliver](../architecture/operators/alert_deliver.md#example-dag-config)

See [dag_configuration.md](dag_configuration.md) for the job field reference.

## Related

- [alert_evaluate_ts](../architecture/operators/alert_evaluate_ts.md)
- [alert_evaluate_py](../architecture/operators/alert_evaluate_py.md)
- [alert_evaluate_rs](../architecture/operators/alert_evaluate_rs.md)
- [alert_deliver](../architecture/operators/alert_deliver.md)
- [data_versioning.md](../architecture/data_versioning.md) — incremental processing and `unique_key` requirements
