# Alerting

User-defined alerts on blockchain data — define conditions, evaluate against live and historical data, deliver notifications.

## Overview

Alerting is a four-stage pipeline:

1. **Definition** — User creates alert rule with conditions and channels
2. **Evaluation** — System evaluates conditions against data (real-time or historical)
3. **Routing** — DAG jobs turn alert events into delivery work items (filters, destinations, staleness gating)
4. **Delivery** — Platform Delivery Service sends notifications and records outcomes

## Alert Definitions

Alert definitions are stored in Postgres data (`alert_definitions`).

PII column: `alert_definitions.channels` (may include email/phone/webhook URLs). Mark it as PII in dataset metadata; see [pii.md](../architecture/data_model/pii.md) for visibility and audit rules.

## Condition Model (UDF)

Users write alert conditions as UDFs. See [udf.md](udf.md) for runtimes, sandbox, resource limits, and determinism requirements.

## Alert Events (Sink)

Triggered alerts are durable facts recorded as append-only rows in `alert_events`.

Multiple jobs/operators may write to this dataset (multi-writer sink). See [ADR 0004](../architecture/adr/0004-alert-event-sinks.md).

In v1, `alert_events` is typically configured as a **buffered Postgres dataset** (SQS buffer → sink → Postgres data). Producers publish records; the platform sink writes the table and emits the upstream dataset event after commit. See [ADR 0006](../architecture/adr/0006-buffered-postgres-datasets.md).

**DAG contract:**
- Producers publish `alert_events` and write with `update_strategy: append` and `unique_key: [dedupe_key]`.
- `dedupe_key` must be deterministic from input data and config. In a multi-writer sink, include a detector identity (e.g., `alert_definition_id` or `producer_job_id`) so different producers don’t dedupe each other (example: `{producer_job_id}:{block_hash}:{tx_hash}`).
- `event_time` must be the contextual “when this happened” time (not task/run time).

## Evaluation (Reference Producers)

Three reference `alert_evaluate` operators evaluate `alert_definitions` and write to `alert_events`:

- `alert_evaluate_ts` (Lambda TypeScript/JavaScript)
- `alert_evaluate_py` (Python: Lambda or `ecs_platform`)
- `alert_evaluate_rs` (Rust: Lambda or `ecs_platform`)

Runtime selection is per-job in the DAG. A single DAG can include many detector jobs (dozens+) across these language implementations and runtimes, all writing to the shared `alert_events` sink.

## Delivery

Operators/UDFs do **not** communicate with the outside world. Delivery is centralized:

- **Routing operators (in the DAG)** read `alert_events`, apply filters + staleness gating, and create `alert_deliveries` work items.
- **Delivery Service (platform service)** is the only component with internet egress for alerting. It leases pending `alert_deliveries`, performs the external send, and records outcomes.

`alert_deliveries` is the durable work queue: one row per `(org_id, alert_event_id, channel)`. Retries update the same row.

Delivery semantics are **at-least-once**. Under timeouts/unknown outcomes, external sends may occur more than once. Trace includes a stable idempotency key (`alert_deliveries.id`) in outbound requests whenever possible.

### Channels

| Channel | Provider | Config |
|---------|----------|--------|
| Email | SES | `to`, `subject_template` |
| SMS | SNS | `phone_number` |
| Webhook | HTTP via Delivery Service | `url`, `headers` |
| Slack | Slack via Delivery Service | `webhook_url`, `channel` |
| PagerDuty | PagerDuty Events API via Delivery Service | `routing_key`, `dedup_key` |

Webhook deliveries are **POST-only** in v1.

Destination validation (SSRF protections) and auditing are enforced by the Delivery Service.
See [delivery_service.md](../architecture/containers/delivery_service.md).

### Routing (Filters)


To route alerts by severity (or any column), run multiple routing jobs with filtered inputs. Filters are read-time predicates on the input edge; see [ADR 0007](../architecture/adr/0007-input-edge-filters.md).

Staleness gating is configured in routing jobs (e.g., `max_delivery_age`) and uses `alert_events.event_time`. Routing should still write `alert_events` for audit (“would have alerted”) even when delivery is suppressed.

```yaml
- name: route_critical
  operator: alert_route
  inputs:
    - from: { dataset: alert_events }
      where: "severity = 'critical'"
  config:
    channel: pagerduty
    max_delivery_age: 7d

- name: route_low
  operator: alert_route
  inputs:
    - from: { dataset: alert_events }
      where: "severity IN ('info','warning')"
  config:
    channel: slack
    max_delivery_age: 7d
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
- [alert_route](../architecture/operators/alert_route.md#example-dag-config)

See [dag_configuration.md](dag_configuration.md) for the job field reference.

## Related

- [alert_evaluate_ts](../architecture/operators/alert_evaluate_ts.md)
- [alert_evaluate_py](../architecture/operators/alert_evaluate_py.md)
- [alert_evaluate_rs](../architecture/operators/alert_evaluate_rs.md)
- [alert_route](../architecture/operators/alert_route.md)
- [data_versioning.md](../architecture/data_versioning.md) — incremental processing and `unique_key` requirements
