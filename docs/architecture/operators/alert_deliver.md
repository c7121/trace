# alert_deliver

Deliver alert events to configured channels.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `lambda` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate |
| **Idle Timeout** | `5m` |
| **Image** | `alert_deliver:latest` |

## Description

Takes alert events and delivers notifications to configured channels (email, SMS, webhook). Handles retries and delivery confirmation.

Optional behavior:
- If `config.channels` is set (e.g., `['pagerduty']`), only those channel types are delivered. This is used for DAG-level routing (e.g., critical → PagerDuty).

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
| Delivery confirmation | External channels | Varies |

## Execution

- **Dependency**: Runs when `alert_events` updates (new alert events)
- **Manual**: Re-deliver failed alerts

## Behavior

- Fetches alert definition and trigger details
- For each configured channel:
  - Format message according to channel type
  - Attempt delivery
  - Record success/failure
- Retries failed deliveries with backoff
- Respects rate limits per channel

## Reliability + Idempotency

- Treats `alert_deliveries` as a durable work queue (one row per `(org_id, alert_event_id, channel)`).
- Claims deliveries with a short lease (`leased_until`) to avoid concurrent sends across replicas and to recover from crashes.
- Sends with an idempotency key: use `alert_deliveries.id` (PagerDuty `dedup_key`, webhook `Idempotency-Key` header).
- Provides exactly-once delivery only when the downstream channel/receiver dedupes on the idempotency key; otherwise delivery is at-least-once under retries/timeouts.

## Channels Supported

| Channel | Provider | Config |
|---------|----------|--------|
| Email | SES (via VPC endpoint) | `to`, `subject_template` |
| SMS | SNS (via VPC endpoint) | `phone_number` |
| Webhook | HTTP (allowlisted URLs) | `url`, `headers`, `method` |
| Slack | Slack API (allowlisted URL) | `webhook_url`, `channel` |
| PagerDuty | PagerDuty Events API (allowlisted) | `routing_key`, `dedup_key` |

## Dependencies

- API keys for channels (injected by Worker wrapper from Secrets Manager)
- Postgres read access (`alert_events`, `alert_definitions`) and write access (`alert_deliveries`)

## Example DAG Config

```yaml
- name: alert_deliver
  activation: reactive
  runtime: lambda
  operator: alert_deliver
  execution_strategy: PerUpdate
  idle_timeout: 5m
  config:
    channels: [slack]
  input_datasets: [alert_events]
  output_datasets: [alert_deliveries]
  update_strategy: replace
  timeout_seconds: 60
```
