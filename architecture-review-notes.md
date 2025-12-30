# Architecture Review Notes

**Date:** December 29, 2025  
**Purpose:** Document review session findings for secondary review

---

## Clarifications / Doc Updates

### 1. Bulk Execution Strategy

**Question:** For `execution_strategy: Bulk`, does the Dispatcher coalesce multiple upstream events into one task, or does it create one task per event (making subsequent tasks no-ops)?

**Discussion:** The three strategies map to:
- `PerUpdate` — stream: task per event
- `PerPartition` — stream of partitions: task per partition key
- `Bulk` — complete batch: single task processes all available work

Creating a task per event for Bulk jobs would be wasteful (all but first are no-ops).

**Conclusion:** Dispatcher coalesces events for Bulk jobs into a single task.

**Recommendation:** Update [data_versioning.md](docs/architecture/data_versioning.md) or [dag_configuration.md](docs/capabilities/dag_configuration.md) to explicitly state Bulk coalescing behavior.

---

### 2. Lambda Runtime Variants

**Question:** Are Lambda sources and Lambda operators both `runtime: lambda`? What about Rust/Python on Lambda?

**Discussion:** Both use `runtime: lambda` — the difference is `activation` (source vs reactive). Future variants anticipated:
- `lambda` (Node)
- `lambda_rs` (Rust custom runtime)
- `lambda_py` (Python)

This matches the ECS pattern (`ecs_rust`, `ecs_python`). ADR 0003 already describes Lambda-style zip bundles for both Rust and Node.

**Conclusion:** Lambda runtime variants are expected but not yet documented.

**Recommendation:** Add `lambda_rs`, `lambda_py` to backlog, or note them as future runtimes in [readme.md](docs/readme.md) Runtime Registry section.

---

### 3. Invalidation Cascading

**Question:** When a downstream job reprocesses due to an upstream invalidation, does it cascade invalidations to *its* outputs?

**Discussion:** Yes, cascading is intended:
1. `block_follower` invalidates `hot_blocks`
2. Downstream jobs reprocess affected range
3. Jobs with `update_strategy: replace` emit invalidations for their outputs
4. Jobs with `update_strategy: append` dedupe via `unique_key`; orphaned rows remain (auditable)
5. Cascade continues until append-only sinks or leaf datasets

**Conclusion:** `replace` jobs cascade invalidations; `append` jobs rely on dedupe.

**Recommendation:** Add explicit statement in [data_versioning.md](docs/architecture/data_versioning.md) Reorg Handling section that `replace` jobs must emit invalidations for their outputs when reprocessing due to upstream invalidation.

---

### 4. Query Service vs Query Operator

**Question:** Are the Query Service and `duckdb_query` operator the same thing? Is Query Service standalone or part of Dispatcher?

**Discussion:** The separation is intentional:
- **Query Service** — thin API layer for interactive queries; executes small queries inline; delegates large queries to operator
- **Query Operator** — standard operator, follows worker contract, swappable

The inline execution is an optimization. The operator is the "real" execution model, enabling future swap to Athena, Trino, etc.

**Conclusion:** Query Service is a standalone service. Query operator should behave exactly like other workers (no special treatment).

**Recommendation:** Review [query_service.md](docs/architecture/query_service.md) for consistency. Ensure operator docs emphasize it follows standard contract and is swappable.

---

### 5. Delivery Service vs alert_deliver Operator

**Question:** The architecture shows both a "Delivery Service" and an `alert_deliver` operator. Are these the same?

**Discussion:** Two separate concerns:
1. **`alert_evaluate`** (operator) — evaluates conditions, writes to `alert_events` (data: "these alerts happened")
2. **Delivery Service** (platform service) — polls `alert_deliveries`, sends to external channels (side effects: "tell the world")

The Delivery Service is like the Dispatcher — a platform service, not a DAG job. It handles crash-safe delivery with leasing and idempotency keys.

**Conclusion:** `alert_deliver` operator materializes `alert_deliveries` rows from `alert_events`. Delivery Service (separate) actually sends notifications.

**Recommendation:** Clarify in [alerting.md](docs/capabilities/alerting.md) that there are two stages: (1) operator creates delivery work items, (2) Delivery Service sends them. Update architecture diagrams if needed.

---

### 6. Backpressure Model

**Question:** For source jobs like `block_follower`, what does "pause" mean under backpressure? The chain doesn't stop producing blocks.

**Discussion:** The Dispatcher is the control point, not the source:
1. Sources always emit events — they don't know about backpressure
2. Dispatcher receives events, creates tasks in Postgres
3. Under pressure: Dispatcher holds tasks in Postgres, doesn't push to SQS
4. Pressure clears: Dispatcher drains accumulated tasks from Postgres → SQS
5. Cursor semantics ensure no data loss — downstream catches up

Postgres is the durable buffer for backpressured work (prevents unbounded SQS growth).

**Conclusion:** Sources keep emitting. Dispatcher absorbs flow and meters it to workers. "Pausing upstream" means Dispatcher stops enqueuing, not sources stop emitting.

**Recommendation:** Update [readme.md](docs/readme.md) Backpressure section to clarify Dispatcher is the valve; sources are decoupled from backpressure.

---

### 7. Sink Implementation

**Question:** For buffered Postgres datasets, is the sink one shared service or one per dataset?

**Discussion:** One sink per dataset (NiFi model):
- Each buffered dataset has its own SQS queue + sink Lambda
- Clean isolation — failure in one doesn't affect others
- Scales independently
- DAG `datasets:` config provisions: SQS queue + sink Lambda (generic code, parameterized)

**Conclusion:** One sink (Lambda + SQS) per buffered dataset.

**Recommendation:** Clarify in [ADR 0006](docs/architecture/adr/0006-buffered-postgres-datasets.md) that each buffered dataset gets its own queue + sink.

---

### 8. Scaling Modes

**Question:** What do `scaling.mode: backfill` vs `steady` mean? Is this about concurrency, priority, or both?

**Discussion:** `backfill` mode was designed for cryo catch-up scenario:
- ~50M blocks to ingest
- Generate many 10K-block partitions
- Parallelize by giving each task a single partition key
- Dispatcher scales workers up to `max_concurrency`

So `backfill` = high parallelism for partition-parallel work; `steady` = normal operation.

Priority shedding ("shed backfill first under pressure") is a separate optimization, not core to scaling mode.

**Conclusion:** `scaling.mode` is about concurrency profile, not priority. Priority shedding is deferrable.

**Recommendation:** Update [dag_configuration.md](docs/capabilities/dag_configuration.md) to clarify `backfill` vs `steady` is about concurrency profile. Move priority shedding to backlog if not already there.

---

## Schema / Structural Changes

### 9. Cross-DAG Dataset Sharing

**Question:** Can jobs in different DAGs share datasets?

**Discussion:** Currently isolated per DAG. But routing is already dataset-name-only (Dispatcher doesn't filter by `dag_name`). The complexity is at deploy time:
- Ownership: who can produce a dataset?
- Naming collisions
- Lifecycle management

Solution: Datasets are global within org. Single producer enforced at deploy. Any job can consume any dataset.

**Conclusion:** Support cross-DAG sharing. Add global dataset registry.

**Recommendation:** Add `datasets` table (see #13). Update [dag_deployment.md](docs/architecture/dag_deployment.md) to validate single-producer at deploy.

---

### 10. Secrets Scoping

**Question:** Should user jobs have access to platform secrets like RPC keys?

**Discussion:** No — workers should never get platform secrets. Need separation:
- `/{env}/{org_slug}/platform/*` — platform secrets, only platform operators
- `/{env}/{org_slug}/user/*` — user secrets, user jobs only

Also need a Secret Writer Service:
- Write-only API (can't read back — prevents exfiltration)
- Role-scoped (who can write to which paths)

**Conclusion:** Separate secret namespaces. Add Secret Writer Service.

**Recommendation:** Update [security_model.md](docs/standards/security_model.md) with:
1. Platform vs user secret paths
2. Secret Writer Service (write-only, role-scoped)
3. Worker scopes fetch based on job type

---

### 11. Task Inputs Schema

**Question:** `tasks.input_versions JSONB` and `task_inputs` table seem redundant. What's the purpose?

**Discussion:** Lost context during review. Both exist:
- `tasks.input_versions` — JSONB on task row
- `task_inputs` — junction table with per-partition detail

May be: one for quick access, one for lineage queries. Or one is redundant.

**Conclusion:** Needs review for redundancy and purpose clarification.

**Recommendation:** Review during implementation. Clarify in [orchestration.md](docs/capabilities/orchestration.md) whether both are needed and their distinct purposes (memoization vs lineage vs both).

---

### 12. Role System Consolidation

**Question:** `users.role` (platform permission) vs `org_roles` (visibility scoping) — why two systems?

**Discussion:** No benefit to two systems. Consolidate:
- **`org_roles`** — single table for all roles
- **System-defined** (reserved): `admin`, `writer`, `reader` — created at org setup
- **Org-defined**: `finance`, `security`, etc. — admin creates
- Drop `users.role` column — permissions come from `org_role_memberships`
- Only `admin` role can assign roles

**Conclusion:** Collapse to single role system.

**Recommendation:** 
1. Update `org_roles` to include system-defined roles
2. Drop `users.role` column
3. Update [orchestration.md](docs/capabilities/orchestration.md), [pii.md](docs/capabilities/pii.md), [security_model.md](docs/standards/security_model.md)
4. Update [erd.md](docs/architecture/erd.md)

---

### 13. Dataset Registry Table

**Question:** Is there a `datasets` table to track dataset metadata?

**Discussion:** The `datasets:` block in DAG YAML declares datasets, but there's no system table. Need:

```sql
CREATE TABLE datasets (
    id UUID PRIMARY KEY,
    org_id UUID NOT NULL REFERENCES orgs(id),
    name TEXT NOT NULL,
    producer_job_id UUID REFERENCES jobs(id),
    storage TEXT NOT NULL,              -- 'postgres' | 's3'
    write_mode TEXT,                    -- 'buffered' | 'direct'
    location TEXT,
    schema JSONB,
    pii_columns TEXT[],
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (org_id, name)
);
```

Used for:
- Single-producer validation at deploy
- Dispatcher looks up metadata when routing
- Query Service finds storage locations
- Cross-DAG sharing (Q9)

**Conclusion:** Add `datasets` table as global registry.

**Recommendation:**
1. Add table to [orchestration.md](docs/capabilities/orchestration.md) or new datasets.md
2. Add to [erd.md](docs/architecture/erd.md)
3. Update [dag_deployment.md](docs/architecture/dag_deployment.md) for deploy-time population
4. Update [dag_configuration.md](docs/capabilities/dag_configuration.md) for YAML schema

---

---

## Dispatcher & Worker Spec Gaps

### 14. Dispatcher Concurrency Model (Answered)

**Question:** Single process, multi-process, or sharded?

**Answer:** Single process for v1. ECS auto-restarts on failure.

**Recommendation:** Document in Dispatcher spec.

---

### 15. Event Ingestion (Answered)

**Question:** How do workers emit events — sync HTTP, bundled with completion, or both?

**Answer:** Both. Workers can emit mid-task via `/internal/events`; task completion includes final events.

**Recommendation:** Document in [contracts.md](docs/architecture/contracts.md).

---

### 16. Lambda Timeout Handling (Answered)

**Question:** Who handles Lambda timeouts — Lambda retry config or Dispatcher reaper?

**Answer:** Dispatcher owns retries (unified across runtimes). Disable Lambda's built-in retries.

**Recommendation:** Document in worker wrapper spec; note Lambda retry config should be disabled.

---

### 17. Worker Wrapper — Lambda vs ECS (Open)

**Question:** Does Lambda receive full task payload in invocation, or just `task_id` (then fetches details)?

**Status:** Deferred — needs further discussion.

**Recommendation:** Spec this before implementing Lambda operators.

---

## Still Needed (Not Yet Spec'd)

| Gap | Priority | Notes |
|-----|----------|-------|
| Dispatcher internal spec | High | API endpoints, state machine, loops |
| Worker wrapper for Lambda | High | Secrets injection, payload shape |
| Secret Writer Service | Medium | Write-only API, role scoping |
| Delivery Service spec | Medium | Polling, leasing, send loop |

---

## Summary of Changes by File

| File | Changes |
|------|---------|
| [readme.md](docs/readme.md) | Backpressure clarification; Lambda runtime variants |
| [data_versioning.md](docs/architecture/data_versioning.md) | Bulk coalescing; invalidation cascading |
| [dag_configuration.md](docs/capabilities/dag_configuration.md) | Scaling modes; dataset registry YAML |
| [dag_deployment.md](docs/architecture/dag_deployment.md) | Single-producer validation; dataset upsert |
| [orchestration.md](docs/capabilities/orchestration.md) | Role consolidation; datasets table; task_inputs review |
| [alerting.md](docs/capabilities/alerting.md) | Delivery Service vs operator clarification |
| [query_service.md](docs/architecture/query_service.md) | Operator follows standard contract |
| [security_model.md](docs/standards/security_model.md) | Secrets scoping; Secret Writer Service; role consolidation |
| [pii.md](docs/capabilities/pii.md) | Role consolidation |
| [erd.md](docs/architecture/erd.md) | datasets table; users.role removal; org_roles update |
| [ADR 0006](docs/architecture/adr/0006-buffered-postgres-datasets.md) | One sink per dataset |
| [backlog.md](docs/plan/backlog.md) | Lambda runtime variants; priority shedding (if not there) |
