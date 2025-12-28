# Entity Relationship Diagram

Complete data model for all system tables.

```mermaid
erDiagram
    ORGS ||--o{ USERS : contains
    ORGS ||--o{ ORG_ROLES : defines
    ORG_ROLES ||--o{ ORG_ROLE_MEMBERSHIPS : includes
    USERS ||--o{ ORG_ROLE_MEMBERSHIPS : assigned
    ORGS ||--o{ JOBS : owns
    JOBS ||--o{ TASKS : creates
    TASKS ||--o{ TASK_INPUTS : records
    JOBS ||--o{ COLUMN_LINEAGE : tracks
    USERS ||--o{ ADDRESS_LABELS : creates
    USERS ||--o{ SAVED_QUERIES : creates
    USERS ||--o{ ALERT_DEFINITIONS : creates
    ALERT_DEFINITIONS ||--o{ ALERT_EVENTS : triggers
    
    ORGS {
        uuid id PK
        text name
        text slug UK
        jsonb settings
        timestamptz created_at
    }
    
    USERS {
        uuid id PK
        uuid org_id FK
        text external_id UK
        text email
        text role
        timestamptz created_at
    }
    
    ORG_ROLES {
        uuid id PK
        uuid org_id FK
        text slug
        text name
        timestamptz created_at
    }
    
    ORG_ROLE_MEMBERSHIPS {
        uuid role_id PK
        uuid user_id PK
        timestamptz created_at
    }
    
    JOBS {
        uuid id PK
        uuid org_id FK
        text name UK
        text dag_name
        text activation
        text runtime
        text operator
        jsonb source
        text execution_strategy
        text idle_timeout
        jsonb config
        text config_hash
        text[] input_datasets
        text output_dataset
        jsonb scaling
        int timeout_seconds
        int heartbeat_timeout_seconds
        int max_attempts
        boolean active
        timestamptz created_at
        timestamptz updated_at
    }
    
    TASKS {
        uuid id PK
        uuid job_id FK
        text status
        text[] partitions
        jsonb input_versions
        text worker_id
        timestamptz started_at
        timestamptz completed_at
        timestamptz last_heartbeat
        int attempts
        timestamptz next_retry_at
        text error_message
        text output_path
        bigint output_row_count
        timestamptz created_at
    }
    
    TASK_INPUTS {
        uuid task_id PK
        text input_dataset PK
        text input_partition PK
        timestamptz input_version
    }
    
    COLUMN_LINEAGE {
        text output_dataset PK
        text output_column PK
        text input_dataset PK
        text input_column PK
        uuid job_id FK
    }
    
    ADDRESS_LABELS {
        uuid id PK
        uuid org_id FK
        uuid user_id FK
        text address
        text label
        text visibility
        timestamptz created_at
        timestamptz updated_at
    }
    
    SAVED_QUERIES {
        uuid id PK
        uuid org_id FK
        uuid user_id FK
        text name
        text query
        text visibility
        timestamptz created_at
        timestamptz updated_at
    }
    
    ALERT_DEFINITIONS {
        uuid id PK
        uuid org_id FK
        uuid user_id FK
        text name
        jsonb condition
        jsonb channels
        text visibility
        boolean enabled
        timestamptz created_at
        timestamptz updated_at
    }
    
    ALERT_EVENTS {
        uuid id PK
        uuid alert_definition_id FK
        text block_hash
        text tx_hash
        bigint block_number
        timestamptz triggered_at
        timestamptz delivered_at
        text delivery_status
    }
    
    PARTITION_VERSIONS {
        text dataset PK
        text partition_key PK
        timestamptz version
        text config_hash
        text schema_hash
        text location
        bigint row_count
        bigint bytes
    }
    
    DATASET_CURSORS {
        text dataset PK
        uuid job_id PK
        text cursor_column
        text cursor_value
        timestamptz updated_at
    }
    
    DATA_INVALIDATIONS {
        uuid id PK
        text dataset
        text scope
        text partition_key
        jsonb row_filter
        text reason
        jsonb source_event
        timestamptz created_at
        uuid[] processed_by
        timestamptz processed_at
    }
    
    PII_ACCESS_LOG {
        uuid id PK
        uuid user_id FK
        text dataset
        text column_name
        uuid record_id
        text action
        timestamptz accessed_at
    }
```

## Schema Sources

Full DDL with constraints and indexes:

| Domain | Tables | Location |
|--------|--------|----------|
| Orchestration | orgs, users, org_roles, org_role_memberships, jobs, tasks, task_inputs, column_lineage | [orchestration.md](../capabilities/orchestration.md) |
| Alerting | alert_definitions, alert_events | [alerting.md](../capabilities/alerting.md) |
| Data Versioning | partition_versions, dataset_cursors, data_invalidations | [data_versioning.md](data_versioning.md) |
| Query Service | saved_queries | [query_service.md](query_service.md) |
| PII | pii_access_log | [pii.md](../capabilities/pii.md) |
| Operators | address_labels | [address_labels.md](operators/address_labels.md) |
