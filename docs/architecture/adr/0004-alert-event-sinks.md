# ADR 0004: Alert Event Sinks (Multi-Writer) + Deliveries

## Status
- Accepted (December 2025)

## Decision
- Alerts are represented as **append-only events** in a shared Postgres sink table: `alert_events`.
- Delivery outcomes are recorded in `alert_deliveries` (one row per alert event + channel), written with **replace/upsert semantics** to support retries without double-sending.
- Multiple jobs/operators may write to `alert_events` (multi-writer) to support many independent “alert checker” producers.
- `alert_events` and `alert_deliveries` are **platform-managed tables** created by migrations on deploy/startup (Dispatcher does not create tables dynamically).
- v1 standardizes a simple severity taxonomy: `info`, `warning`, `critical`.

## Context
- “Alerts that happened” are first-class facts and must be durable, queryable, and auditable.
- Many producers can generate alerts (UDF evaluators, system monitors, integrity checks, custom operators).
- Delivery is side-effectful and retried; tracking delivery history should not require mutating the alert facts.

## Why
- **Multi-producer support**: a shared sink avoids forcing every producer to own a unique output table.
- **Immutability**: append-only facts simplify retries, replay, and auditing.
- **Separation of concerns**: triggering vs. delivery have different lifecycles and failure modes.
- **Operational clarity**: platform-owned tables keep bootstrap simple and consistent across environments.

## Contract
- In DAG YAML, alert-producing jobs write `output_datasets: [alert_events]` with `update_strategy: append`.
- Deduplication uses a deterministic key:
  - `alert_events.dedupe_key` is required.
  - `UNIQUE (org_id, dedupe_key)` enforces idempotency across retries and reprocessing.
- Delivery jobs read `input_datasets: [alert_events]` and write `output_datasets: [alert_deliveries]` with `update_strategy: replace`.

## Consequences
- `alert_events` is the canonical source for “alerts that happened”.
- `alert_deliveries` is the canonical source for “actions taken” (PagerDuty/Slack/Email/Webhook).
- Per-alert delivery status is computed via query (join), not stored as mutable columns on `alert_events`.
- “Evaluation logs” are treated as operator logs/metrics in v1 (not a separate dataset/table unless added explicitly later).

## Open Questions
- Delivery destination storage (store full destination vs. store hashes/pointers; impacts PII tagging).
