# ERD - Postgres state

Relationships for orchestration, lineage, and dataset registry.

Canonical DDL lives in `harness/migrations/state/` (applied in order). This ERD is intentionally relationship-focused and omits most columns to reduce drift.

For a column-level sketch, see [`state_schema.md`](state_schema.md).

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
    DATASET_VERSIONS ||--o{ PARTITION_VERSIONS : partitions
    DATASETS ||--o{ PARTITION_VERSIONS : partitions
    DATASET_VERSIONS ||--o{ DATASET_CURSORS : cursors
    DATASETS ||--o{ DATASET_CURSORS : cursors
    JOBS ||--o{ DATASET_CURSORS : advances
    DATASET_VERSIONS ||--o{ DATA_INVALIDATIONS : invalidations
    DATASETS ||--o{ DATA_INVALIDATIONS : invalidations
```
