# ADR 0004: Observability Stack

## Status
- Accepted

## Decision
- Use **AWS CloudWatch** as the default metrics/logs/alerts backend, with the option to add/dual-home to Datadog or Prometheus/Grafana later.

## Why
- Native AWS integration; easy to manage via Terraform; fits SOC2 needs; keeps Day 1 simple.
- Pluggable path for richer UX or existing org standards.

## Consequences
- Publish metrics/logs/traces to CloudWatch; set dashboards/alarms for key SLOs.
- Wire alerts via email/SMS/webhook for system issues.
- If adding another provider, keep exporters/shims in place to avoid app changes.

## Open Questions
- None currently.
