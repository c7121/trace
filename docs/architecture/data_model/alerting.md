# Alerting Data Model

Schema mapping notes for alerting tables.

> These tables live in **Postgres data**. Columns like `org_id`/`user_id` and producer ids refer to entities in **Postgres state** and are **soft references** (no cross-DB foreign keys).

Where to look:
- Columns: [data_schema.md](data_schema.md)
- Implemented DDL: `harness/migrations/data/`

## data.alert_events (implemented in harness)

The contract-freeze harness uses a minimal `data.alert_events` sink table.

- Canonical DDL: `harness/migrations/data/0001_init.sql`
- Invariants:
  - Idempotency key: `dedupe_key` is the primary key.
  - `event_time` is required and should represent the alert's domain time.

## data.alert_definitions (planned)

User-managed alert definitions.

- Invariants:
  - `visibility` uses the same levels as other user-managed datasets (see [pii.md](pii.md)).
  - `condition` is stored as JSON and evaluated by the alerting system.
  - `channels` is stored as JSON (email, SMS, webhook, etc).

## data.alert_events (planned platform schema)

The full platform alerting schema extends the harness sink shape to include org scoping, producer identity, and richer onchain context.

- Invariants:
  - Uniqueness: `(org_id, dedupe_key)` (deterministic idempotency key).
  - `event_time` is the alert's domain time and is used for staleness gating.
  - Chain context fields like `block_hash` and `tx_hash` may be `NULL` depending on alert type.

## data.alert_deliveries (planned)

Crash-safe delivery attempts for alert events.

- Invariants:
  - Uniqueness: `(org_id, alert_event_id, channel)`
  - Leasing is time-bounded (`leased_until`) to prevent duplicate concurrent delivery.
  - Ready-to-send is driven by `(status, next_attempt_at)`.

## Related

- [alerting.md](../../specs/alerting.md) - alerting behavior and flows
- [pii.md](pii.md) - visibility and audit rules
