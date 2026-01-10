# ERD - Postgres data

Relationships for platform-owned data-plane tables.

Canonical DDL lives in `harness/migrations/data/` (applied in order). This ERD is intentionally relationship-focused and omits most columns to reduce drift.

For a column-level sketch, see [`data_schema.md`](data_schema.md).

> Note: `org_id`/`user_id`/`task_id` are **soft references** to Postgres state (no cross-DB FKs).

```mermaid
erDiagram
    SAVED_QUERIES ||--o{ QUERY_RESULTS : executes
    ALERT_DEFINITIONS ||--o{ ALERT_EVENTS : triggers
    ALERT_EVENTS ||--o{ ALERT_DELIVERIES : delivers

    %% Standalone tables - soft references to Postgres state
    ADDRESS_LABELS {
        uuid org_id
        uuid user_id
    }
    QUERY_AUDIT {
        uuid org_id
        uuid task_id
        uuid dataset_id
    }
    USER_QUERY_AUDIT {
        uuid org_id
        text user_sub
        uuid dataset_id
    }
    PII_ACCESS_LOG {
        uuid org_id
        uuid user_id
        uuid task_id
    }
```
