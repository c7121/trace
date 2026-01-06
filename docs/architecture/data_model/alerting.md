# Alerting Data Model

Canonical DDL for alerting tables.

> These tables live in **Postgres data**. Columns like `org_id`/`user_id` and producer ids refer to entities in **Postgres state** and are **soft references** (no cross-DB foreign keys).

## alert_definitions

```sql
CREATE TABLE alert_definitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL, -- soft ref: Postgres state orgs(id)
    user_id UUID NOT NULL, -- soft ref: Postgres state users(id)
    name TEXT NOT NULL,
    condition JSONB NOT NULL,         -- UDF or expression (see below)
    channels JSONB NOT NULL,          -- email, sms, webhook configs
    visibility TEXT NOT NULL DEFAULT 'private',  -- see pii.md
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);
```

## alert_events

```sql
CREATE TABLE alert_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL, -- soft ref: Postgres state orgs(id)
    alert_definition_id UUID REFERENCES alert_definitions(id), -- nullable for non-UDF/system alerts
    producer_job_id UUID, -- soft ref: Postgres state jobs(id)
    producer_task_id UUID, -- soft ref: Postgres state tasks(id)
    severity TEXT,                      -- e.g., 'info'|'warning'|'critical'
    chain_id BIGINT,
    block_number BIGINT,
    block_hash TEXT,                    -- changes on reorg
    tx_hash TEXT,                       -- nullable for block-level alerts
    source_dataset_uuid UUID,           -- upstream dataset (optional)
    partition_key TEXT,                 -- e.g., '1000000-1010000' (optional)
    cursor_value TEXT,                  -- e.g., block height cursor (optional)
    payload JSONB NOT NULL DEFAULT '{}',-- producer-defined details
    dedupe_key TEXT NOT NULL,           -- deterministic idempotency key
    event_time TIMESTAMPTZ NOT NULL,    -- domain time (e.g., block_timestamp); used for staleness gating
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (org_id, dedupe_key)
);
```

## alert_deliveries

```sql
CREATE TABLE alert_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL, -- soft ref: Postgres state orgs(id)
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

## Related

- [alerting.md](../../specs/alerting.md) - alerting behavior and flows
- [pii.md](pii.md) - visibility and audit rules