# alert_route

Create alert delivery work items for the platform Delivery Service.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `ecs_platform` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate |
| **Idle Timeout** | `5m` |
| **Image** | `alert_route:latest` |

## Description

Reads `alert_events` and creates `alert_deliveries` work items (rows) for a configured channel/route. Applies routing filters and staleness gating, but does **not** perform any external sends.

External delivery is handled by a platform Delivery Service which leases `alert_deliveries`, sends to providers, and updates delivery status.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `cursor` | event | Cursor value from upstream `alert_events` event (e.g., `block_number`) |
| `alert_events` | storage | Postgres table of alert events (read) |
| `alert_definitions` | storage | Postgres table of definitions/channels (read) |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Delivery status | `postgres://alert_deliveries` | Rows |

## Execution

- Runs when `alert_events` updates (new alert events)
- May be re-run for repair or replay; it must remain idempotent

## Behavior

- Applies an optional input edge filter (`where`) to select which alert events to route.
  - `where` is a **structured filter map**, not an arbitrary SQL predicate. See `docs/specs/dag_configuration.md`.
- Applies staleness gating using the alert’s contextual time (e.g., `alert_events.event_time`) vs `config.max_delivery_age`.
- Creates `alert_deliveries` rows idempotently (unique key `(org_id, alert_event_id, channel)`).
- Does not attempt delivery, retry, or rate limit. Those are Delivery Service responsibilities.

## Reliability + Idempotency

- Treats `alert_deliveries` as a durable work queue (one row per `(org_id, alert_event_id, channel)`).
- Creating deliveries is idempotent via unique constraints/upserts.
- Exactly-once delivery is conditional on downstream/provider deduplication and is handled by the Delivery Service (not this operator).

## Channels

`config.channel` selects the channel type for deliveries created by this operator (e.g., `slack`, `pagerduty`).

## Dependencies

- Postgres read access (`alert_events`, `alert_definitions`) and write access (`alert_deliveries`)
- No external API keys required (Delivery Service holds and uses them).

## Example DAG Config

```yaml
- name: route_critical
  activation: reactive
  runtime: ecs_platform
  operator: alert_route
  execution_strategy: PerUpdate
  idle_timeout: 5m
  inputs:
    - from: { dataset: alert_events }
      where:
        severity: critical
  outputs: 1
  update_strategy: append
  unique_key: [alert_event_id, channel]
  config:
    channel: pagerduty
    max_delivery_age: 7d
  timeout_seconds: 60
```
