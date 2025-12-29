# alert_deliver

Deliver triggered alerts to configured channels.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `lambda` |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate |
| **Idle Timeout** | `5m` |
| **Image** | `alert_deliver:latest` |

## Description

Takes triggered alert events and delivers notifications to configured channels (email, SMS, webhook). Handles retries and delivery confirmation.

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `alert_id` | partition | The triggered alert to deliver |
| `channels` | config (from alert_definitions) | Delivery channels |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Delivery status | `postgres://alert_deliveries` | Rows |
| Delivery confirmation | External channels | Varies |

## Execution

- **Dependency**: Runs after alert_evaluate produces triggered alerts
- **Manual**: Re-deliver failed alerts

## Behavior

- Fetches alert definition and trigger details
- For each configured channel:
  - Format message according to channel type
  - Attempt delivery
  - Record success/failure
- Retries failed deliveries with backoff
- Respects rate limits per channel

## Channels Supported

| Channel | Provider | Config |
|---------|----------|--------|
| Email | SES (via VPC endpoint) | `to`, `subject_template` |
| SMS | SNS (via VPC endpoint) | `phone_number` |
| Webhook | HTTP (allowlisted URLs) | `url`, `headers`, `method` |
| Slack | Slack API (allowlisted URL) | `webhook_url`, `channel` |

## Dependencies

- API keys for channels (injected by Worker wrapper from Secrets Manager)
- Postgres read/write access

## Example DAG Config

```yaml
- name: alert_deliver
  activation: reactive
  runtime: lambda
  operator: alert_deliver
  execution_strategy: PerUpdate
  idle_timeout: 5m
  config: {}
  input_datasets: [triggered_alerts]
  output_datasets: [alert_deliveries]
  timeout_seconds: 60
```
