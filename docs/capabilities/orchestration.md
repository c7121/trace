# Orchestration Data Model

Core schemas for job orchestration, task management, and lineage tracking.

## Organizations

```sql
CREATE TABLE orgs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    settings JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT now()
);
```

## Users

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    external_id TEXT NOT NULL UNIQUE,  -- IdP subject
    email TEXT,
    role TEXT NOT NULL,                -- platform permission role: 'reader', 'writer', 'admin'
    created_at TIMESTAMPTZ DEFAULT now()
);
```

## Org Roles (User-Defined)

Org-defined roles used for visibility scoping (e.g., `role:finance`).

```sql
CREATE TABLE org_roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    slug TEXT NOT NULL,                -- e.g., 'finance'
    name TEXT NOT NULL,                -- e.g., 'Finance'
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (org_id, slug)
);
```

## Org Role Memberships

```sql
CREATE TABLE org_role_memberships (
    role_id UUID NOT NULL REFERENCES org_roles(id),
    user_id UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (role_id, user_id)
);

CREATE INDEX idx_org_role_memberships_user ON org_role_memberships(user_id);
```

## Jobs

Job definitions synced from YAML DAG config.

```sql
CREATE TABLE jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    name TEXT NOT NULL,
    dag_name TEXT NOT NULL,
    activation TEXT NOT NULL,           -- 'source', 'reactive'
    runtime TEXT NOT NULL,              -- 'lambda', 'ecs_rust', 'ecs_python', 'dispatcher'
    operator TEXT NOT NULL,             -- 'block_follower', 'alert_evaluate', etc.
    source JSONB,                       -- { "kind": "cron", "schedule": "0 0 * * *" }
    execution_strategy TEXT,            -- NULL for sources, else 'PerUpdate' | 'PerPartition' (Bulk is not supported)
    idle_timeout TEXT,                  -- reactive only: 'never', '5m', '0', etc.
    config JSONB NOT NULL DEFAULT '{}', -- operator config
    config_hash TEXT NOT NULL,
    input_datasets TEXT[],              -- upstream edges resolved to dataset UUIDs (string form)
    output_datasets TEXT[],             -- output dataset UUIDs by output index (string form)
    update_strategy TEXT NOT NULL,      -- 'append' | 'replace'
    unique_key TEXT[],                  -- required if update_strategy = 'append'
    scaling JSONB,                      -- { "mode": "backfill", "max_concurrency": 20 }
    timeout_seconds INT,
    heartbeat_timeout_seconds INT,
    max_attempts INT DEFAULT 3,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    CONSTRAINT unique_job_name UNIQUE (dag_name, name)
);

CREATE INDEX idx_jobs_active ON jobs(dag_name) WHERE active = true;
```

## Datasets (Registry)

Published datasets are registered for discovery and querying. The registry:

- Maps `dataset_name` (human-readable, unique per org) → `dataset_uuid` (system UUID primary key).
- Links datasets back to their producer (`dag_name`, `job_name`, `output_index`) for navigation and “single producer” enforcement.

```sql
CREATE TABLE datasets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(), -- dataset_uuid
    org_id UUID NOT NULL REFERENCES orgs(id),
    name TEXT NOT NULL,                            -- dataset_name (unique per org)
    producer_dag_name TEXT NOT NULL,
    producer_job_name TEXT NOT NULL,
    producer_output_index INT NOT NULL,
    storage JSONB NOT NULL DEFAULT '{}',            -- { "backend": "postgres"|"s3", "location": "...", ... }
    read_roles TEXT[] NOT NULL DEFAULT '{}',        -- admin-managed visibility
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (org_id, name),
    UNIQUE (org_id, producer_dag_name, producer_job_name, producer_output_index)
);
```

### Runtime Execution Model

The `runtime` field determines how the Dispatcher executes a task:

- `ecs_*`: Dispatcher enqueues `task_id` to an SQS task queue; ECS workers long-poll, execute, then report completion.
- `lambda`: Dispatcher invokes a Lambda directly (no SQS) with the **full task payload** (same shape as `/internal/task-fetch`); the Lambda runs without database credentials and reports completion via the same Dispatcher endpoints.
- `dispatcher`: Dispatcher runs the operator in-process (no SQS, no Lambda).

See [readme.md](../readme.md) for diagrams and [contracts.md](../architecture/contracts.md) for the invocation payload shape.

## Tasks

Task instances (append-only).

```sql
CREATE TABLE tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES jobs(id),
    status TEXT NOT NULL DEFAULT 'Queued',
    partitions TEXT[],
    input_versions JSONB,
    worker_id TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    last_heartbeat TIMESTAMPTZ,
    attempts INT DEFAULT 0,
    next_retry_at TIMESTAMPTZ,
    error_message TEXT,
    outputs JSONB,                      -- per-dataset outputs (paths, cursors, partitions, metrics)
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_tasks_status ON tasks(status) WHERE status IN ('Queued', 'Running');
CREATE INDEX idx_tasks_job_id ON tasks(job_id);
CREATE INDEX idx_tasks_next_retry ON tasks(next_retry_at) WHERE status = 'Failed';
CREATE INDEX idx_tasks_last_heartbeat ON tasks(last_heartbeat) WHERE status = 'Running';
```

## Task Inputs

Task input versions for memoization.

```sql
CREATE TABLE task_inputs (
    task_id UUID REFERENCES tasks(id),
    input_dataset_uuid UUID NOT NULL REFERENCES datasets(id),
    input_partition_key TEXT NOT NULL,
    input_version TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (task_id, input_dataset_uuid, input_partition_key)
);
```

## Column Lineage (Optional)

```sql
CREATE TABLE column_lineage (
    output_dataset_uuid UUID NOT NULL REFERENCES datasets(id),
    output_column TEXT NOT NULL,
    input_dataset_uuid UUID NOT NULL REFERENCES datasets(id),
    input_column TEXT NOT NULL,
    job_id UUID REFERENCES jobs(id),
    PRIMARY KEY (output_dataset_uuid, output_column, input_dataset_uuid, input_column)
);
```

## Task Lifecycle

The `status` field tracks task state:

```mermaid
stateDiagram-v2
    [*] --> Queued: Task created
    Queued --> Running: Worker picks up
    Running --> Completed: Success
    Running --> Failed: Error / Timeout
    Failed --> Queued: Retry (if attempts < max)
    Failed --> [*]: Max retries exceeded
    Completed --> [*]
```

**Retry behavior:** Failed tasks retry up to `max_attempts`. The `next_retry_at` field schedules retries with backoff.

**Heartbeats:** Running tasks update `last_heartbeat`. Tasks exceeding `heartbeat_timeout_seconds` without a heartbeat are marked Failed by the reaper.

## Source Job Lifecycle

Jobs with `activation: source` run continuously:

```mermaid
stateDiagram-v2
    [*] --> Pending: Job deployed
    Pending --> Running: Dispatcher ensures running
    Running --> Running: Heartbeat OK
    Running --> Dead: Heartbeat timeout
    Dead --> Pending: Dispatcher restarts
    Running --> Draining: Shutdown signal
    Draining --> [*]: Graceful stop
```

## Reactive Job Lifecycle

Jobs with `activation: reactive` scale based on events:

```mermaid
stateDiagram-v2
    [*] --> Idle: No events
    Idle --> Running: Event received
    Running --> Running: More events
    Running --> Idle: idle_timeout exceeded
    Idle --> [*]: Scale to zero (optional)
```

## Related

- [Architecture Overview](../readme.md) — system design and component diagrams
- [DAG Configuration](dag_configuration.md) — YAML schema
- [DAG Deployment](../architecture/dag_deployment.md) — deploy/sync flow
