# Entity Relationship Diagram

Complete data model for all system tables.

Note: `jobs.input_datasets` / `jobs.output_datasets` store internal dataset UUIDs (string form), not user-facing dataset names.

```mermaid
erDiagram
    ORGS ||--o{ USERS : contains
    ORGS ||--o{ ORG_ROLES : defines
    ORG_ROLES ||--o{ ORG_ROLE_MEMBERSHIPS : includes
    USERS ||--o{ ORG_ROLE_MEMBERSHIPS : assigned
    ORGS ||--o{ DAG_VERSIONS : owns
    DAG_VERSIONS ||--o{ DAG_CURRENT_VERSIONS : serves
    DAG_VERSIONS ||--o{ JOBS : defines
    ORGS ||--o{ JOBS : owns
    ORGS ||--o{ DATASETS : owns
    DATASETS ||--o{ DATASET_VERSIONS : versions
    DAG_VERSIONS ||--o{ DAG_VERSION_DATASETS : pins
    DATASET_VERSIONS ||--o{ DAG_VERSION_DATASETS : referenced_by
    JOBS ||--o{ TASKS : creates
    TASKS ||--o{ TASK_INPUTS : records
    JOBS ||--o{ COLUMN_LINEAGE : tracks
    USERS ||--o{ ADDRESS_LABELS : creates
    USERS ||--o{ SAVED_QUERIES : creates
    USERS ||--o{ QUERY_RESULTS : runs
    USERS ||--o{ ALERT_DEFINITIONS : creates
    ALERT_DEFINITIONS ||--o{ ALERT_EVENTS : triggers
    ALERT_EVENTS ||--o{ ALERT_DELIVERIES : delivers
    ORGS ||--o{ ALERT_EVENTS : owns
    ORGS ||--o{ ALERT_DELIVERIES : owns
    ORGS ||--o{ QUERY_RESULTS : owns
    SAVED_QUERIES ||--o{ QUERY_RESULTS : executes
    TASKS ||--o{ QUERY_RESULTS : records
    
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

    DAG_VERSIONS {
        uuid id PK
        uuid org_id FK
        text dag_name
        text yaml_hash
        timestamptz created_at
    }

    DAG_CURRENT_VERSIONS {
        uuid org_id PK
        text dag_name PK
        uuid dag_version_id FK
        timestamptz updated_at
    }
    
    JOBS {
        uuid id PK
        uuid org_id FK
        text name
        text dag_name
        uuid dag_version_id FK
        text activation
        text runtime
        text operator
        jsonb source
        text execution_strategy
        text idle_timeout
        jsonb config
        text config_hash
        text[] input_datasets
        text[] output_datasets
        text update_strategy
        text[] unique_key
        jsonb scaling
        int timeout_seconds
        int heartbeat_timeout_seconds
        int max_attempts
        timestamptz created_at
        timestamptz updated_at
    }

    DATASETS {
        uuid id PK
        uuid org_id FK
        text name
        text producer_dag_name
        text producer_job_name
        int producer_output_index
        jsonb storage
        text[] read_roles
        timestamptz created_at
        timestamptz updated_at
    }

    DATASET_VERSIONS {
        uuid id PK
        uuid dataset_uuid FK
        timestamptz created_at
        text storage_location
        text config_hash
        text schema_hash
    }

    DAG_VERSION_DATASETS {
        uuid dag_version_id PK
        uuid dataset_uuid PK
        uuid dataset_version FK
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
        jsonb outputs
        timestamptz created_at
    }
    
    TASK_INPUTS {
        uuid task_id PK
        uuid input_dataset_uuid PK
        text input_partition_key PK
        timestamptz input_version
    }
    
    COLUMN_LINEAGE {
        uuid output_dataset_uuid PK
        text output_column PK
        uuid input_dataset_uuid PK
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

    QUERY_RESULTS {
        uuid id PK
        uuid org_id FK
        uuid user_id FK
        uuid saved_query_id FK
        uuid task_id FK
        text mode
        text status
        text sql_hash
        text output_format
        text output_location
        bigint row_count
        bigint bytes
        int duration_ms
        text error_code
        text error_message
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
        uuid org_id FK
        uuid alert_definition_id FK
        uuid producer_job_id FK
        uuid producer_task_id FK
        text severity
        bigint chain_id
        text block_hash
        text tx_hash
        bigint block_number
        uuid source_dataset_uuid
        text partition_key
        text cursor_value
        jsonb payload
        text dedupe_key
        timestamptz event_time
        timestamptz created_at
    }

    ALERT_DELIVERIES {
        uuid id PK
        uuid org_id FK
        uuid alert_event_id FK
        text channel
        text status
        int attempt
        timestamptz next_attempt_at
        timestamptz leased_until
        text lease_owner
        timestamptz last_attempt_at
        text provider_message_id
        text error_message
        timestamptz delivered_at
        timestamptz created_at
        timestamptz updated_at
    }
    
    PARTITION_VERSIONS {
        uuid dataset_uuid PK
        uuid dataset_version PK
        text partition_key PK
        timestamptz materialized_at
        text config_hash
        text schema_hash
        text location
        bigint row_count
        bigint bytes
    }
    
    DATASET_CURSORS {
        uuid dataset_uuid PK
        uuid dataset_version PK
        uuid job_id PK
        text cursor_column
        text cursor_value
        timestamptz updated_at
    }
    
    DATA_INVALIDATIONS {
        uuid id PK
        uuid dataset_uuid FK
        uuid dataset_version FK
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
| Orchestration | orgs, users, org_roles, org_role_memberships, dag_versions, dag_current_versions, jobs, tasks, task_inputs, column_lineage, datasets, dataset_versions, dag_version_datasets | [orchestration.md](orchestration.md) |
| Alerting | alert_definitions, alert_events, alert_deliveries | [alerting.md](../../features/alerting.md) |
| Data Versioning | partition_versions, dataset_cursors, data_invalidations | [data_versioning.md](../data_versioning.md) |
| Query Service | saved_queries, query_results | [query_service.md](../containers/query_service.md) |
| PII | pii_access_log | [pii.md](pii.md) |
| Operators | address_labels | [address_labels.md](../operators/address_labels.md) |
