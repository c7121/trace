# ADR 0004: Alert Event Sinks (Multi-Writer) + Deliveries

## Status
- Accepted (December 2025)

## Decision
- Alerts are represented as **append-only events** in a shared Postgres data sink table: `alert_events`.
- Producers publish alert records via the **buffered Postgres dataset** mechanism (SQS buffer → sink → Postgres data); see [ADR 0006](0006-buffered-postgres-datasets.md).
- Delivery work and outcomes are recorded in `alert_deliveries` (one row per alert event + channel), written with **replace/upsert semantics** to support retries without double-sending.
- Operators/UDFs do **not** perform external calls; delivery is handled by a platform **Delivery Service** that leases pending `alert_deliveries`, performs the external send, and updates delivery status.
- Delivery uses `alert_deliveries.id` as an **idempotency key** per `(alert_event_id, channel)`; exactly-once delivery is conditional on downstream/provider support for deduplication.
- Multiple jobs/operators may write to `alert_events` (multi-writer) to support many independent “alert checker” producers. In v1, this is intended within a single DAG (cross-DAG shared writes remain disallowed unless explicitly enabled later).
- `alert_events` and `alert_deliveries` are **platform-created tables** on deploy/startup (schema declared in config; producers do not create tables dynamically).
- Webhook/email/phone destinations live in `alert_definitions.channels` (PII). `alert_deliveries` stores only channel type and provider response metadata (no destination duplication).
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
- In DAG YAML, alert-producing jobs write to `alert_events` with `update_strategy: append` and a deterministic `dedupe_key`.
- Deduplication uses a deterministic key:
  - `alert_events.dedupe_key` is required.
  - `UNIQUE (org_id, dedupe_key)` enforces idempotency across retries and reprocessing.
- In a multi-writer sink, `dedupe_key` must include a detector identity (e.g., `alert_definition_id` or `producer_job_id`) so one detector cannot suppress another via deduplication.
- One or more routing jobs read `alert_events` (optionally filtered) and create `alert_deliveries` work items (one per `(alert_event_id, channel)`), applying policy like staleness gating.
- The Delivery Service performs the external send and updates `alert_deliveries` status/attempt fields.

## Consequences
- `alert_events` is the canonical source for “alerts that happened”.
- `alert_deliveries` is the canonical source for “actions taken” (PagerDuty/Slack/Email/Webhook).
- Per-alert delivery status is computed via query (join), not stored as mutable columns on `alert_events`.
- “Evaluation logs” are treated as operator logs/metrics in v1 (not a separate dataset/table unless added explicitly later).
