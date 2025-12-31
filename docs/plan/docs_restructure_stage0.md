# Docs Restructure (Stage 0 Artifact)

This document captures the **target end-state structure** and a **no-information-loss mapping** from current docs to their destinations. It is review-only; subsequent stages apply changes one at a time with explicit approval.

## Decisions (approved)

- Keep **snake_case** filenames and directories.
- Rename `docs/capabilities/` → `docs/features/` (Stage 3).
- Move `docs/capabilities/pii.md` → `docs/architecture/data_model/pii.md` (Stage 2).

## Target Directory Structure (End State)

```text
docs/
  readme.md                      # ~150–200 lines: overview, C4 L1+L2, doc map
  _legacy/                       # temporary snapshots during migration
    readme_YYYYMMDD.md

  architecture/
    adr/                         # existing ADRs (keep)
    operators/                   # existing operator docs (keep)
    containers/                  # per-container internals (new)
      dispatcher.md
      gateway.md
      workers.md
      query_service.md
    data_model/                  # DDL + data/policy model docs (new)
      erd.md
      orchestration.md
      pii.md
    contracts.md                 # existing (keep)
    data_versioning.md           # existing (keep)
    dag_deployment.md            # existing (keep)
    event_flow.md                # new (from docs/readme.md)

  features/                      # Stage 3 rename from capabilities/
    alerting.md
    dag_configuration.md
    ingestion.md
    metadata.md
    udf.md

  deploy/                        # new
    infrastructure.md
    monitoring.md

  standards/                     # existing (keep)
    nfr.md
    security_model.md

  use_cases/                     # existing (keep)
  plan/                          # existing (keep)
  prd/                           # existing (keep)
```

## Target `docs/readme.md` (End State) Outline

**Goal:** keep `docs/readme.md` as the “front door” (C4 L1/L2 + doc map) and move deeper detail into single-source docs.

### Sections that stay (in `docs/readme.md`)

- **Overview** (short)
- **Design Principles** (keep the current 5 bullets)
- **Concepts**
  - **Glossary** table (keep)
  - **Job Types** table (keep)
- **Architecture**
  - **System Context** (C4 L1 mermaid) (keep)
  - **Container View** (C4 L2 mermaid) (keep; update container list to be accurate/consistent)
  - **Storage** (short; preserve the “hot vs cold is a convention” nuance and DuckDB federation summary)
- **Documentation Map** (table of links to the docs below)
- **Getting Started** (links to deploy docs / build plan)

### Sections that move out (from `docs/readme.md`)

| Topic in current `docs/readme.md` | Destination (end state) |
|---|---|
| Event flow sequence diagram | `docs/architecture/event_flow.md` |
| Dispatcher internals (routing, backpressure, failure mode) | `docs/architecture/containers/dispatcher.md` |
| Worker runtime model (SQS polling, heartbeats, lambda vs ECS) | `docs/architecture/containers/workers.md` |
| Query Service component view diagram (if kept) | `docs/architecture/containers/query_service.md` |
| Monitoring “key alerts” list | `docs/deploy/monitoring.md` |
| “Why SQS over Postgres-as-queue” rationale | `docs/architecture/containers/dispatcher.md` (optional later ADR) |
| Appendix references (Cryo/DuckDB/AWS links) | Inline under the most relevant docs (features/ingestion, architecture/query_service, deploy/infrastructure) |

## Full Mapping Table (Current → Destination)

### A) File Moves / Renames

| Current | Destination (end state) | Stage | Notes |
|---|---|---:|---|
| `README.md` | `README.md` | 3–4 | Update doc links after moves/rename. |
| `docs/readme.md` | `docs/readme.md` | 4 | Trim + move deep sections out; keep a verbatim snapshot in `docs/_legacy/` first. |
| `docs/capabilities/alerting.md` | `docs/features/alerting.md` | 3 | Pure rename/move (content preserved). |
| `docs/capabilities/dag_configuration.md` | `docs/features/dag_configuration.md` | 3 | Pure rename/move (content preserved). |
| `docs/capabilities/ingestion.md` | `docs/features/ingestion.md` | 3 | Pure rename/move (content preserved). |
| `docs/capabilities/metadata.md` | `docs/features/metadata.md` | 3 | Pure rename/move (content preserved). |
| `docs/capabilities/udf.md` | `docs/features/udf.md` | 3 | Pure rename/move (content preserved). |
| `docs/capabilities/gateway.md` | `docs/architecture/containers/gateway.md` | 2 | Container doc (external entrypoint). |
| `docs/capabilities/infrastructure.md` | `docs/deploy/infrastructure.md` | 2 | Deployment/infra doc. |
| `docs/capabilities/orchestration.md` | `docs/architecture/data_model/orchestration.md` | 2 | Data model / schema reference. |
| `docs/capabilities/pii.md` | `docs/architecture/data_model/pii.md` | 2 | Data model + policy (per decision). |
| `docs/architecture/query_service.md` | `docs/architecture/containers/query_service.md` | 2 | Group under containers (content preserved). |
| `docs/architecture/erd.md` | `docs/architecture/data_model/erd.md` | 2 | Group under data_model (content preserved). |
| `docs/architecture/contracts.md` | `docs/architecture/contracts.md` | 4 | Keep; add/adjust links to `event_flow.md` as needed. |
| `docs/architecture/data_versioning.md` | `docs/architecture/data_versioning.md` | — | No move planned. |
| `docs/architecture/dag_deployment.md` | `docs/architecture/dag_deployment.md` | — | No move planned. |
| `docs/architecture/adr/*` | `docs/architecture/adr/*` | — | No move planned. |
| `docs/architecture/operators/*` | `docs/architecture/operators/*` | — | No move planned. |
| `docs/standards/*` | `docs/standards/*` | — | No move planned. |
| `docs/use_cases/*` | `docs/use_cases/*` | — | No move planned. |
| `docs/plan/backlog.md` | `docs/plan/backlog.md` | — | No move planned. |
| `docs/prd/prd.md` | `docs/prd/prd.md` | — | No move planned. |
| (new) | `docs/architecture/event_flow.md` | 4 | Extract from `docs/readme.md` “Event Flow”. |
| (new) | `docs/architecture/containers/dispatcher.md` | 4 | Extract Dispatcher-related sections from `docs/readme.md`. |
| (new) | `docs/architecture/containers/workers.md` | 4 | Extract Worker-related sections from `docs/readme.md`. |
| (new) | `docs/deploy/monitoring.md` | 4 | Extract Monitoring section from `docs/readme.md`. |

### B) `docs/readme.md` Section Moves (by heading)

| Current section | Destination (end state) | Notes |
|---|---|---|
| `docs/readme.md:1 # ETL Orchestration System Architecture` | `docs/readme.md` | Title likely updated, content preserved via moves below. |
| `docs/readme.md:8 ## Table of Contents` | (remove from `docs/readme.md`) | Replaced by “Documentation Map”. |
| `docs/readme.md:23 ## Overview` | `docs/readme.md` | Keep, shorten. |
| `docs/readme.md:35 ### Design Principles` | `docs/readme.md` | Keep as-is (5 bullets). |
| `docs/readme.md:43 ### Tenancy Model` | `docs/readme.md` | Keep (short). |
| `docs/readme.md:47 ### Job Characteristics` | `docs/readme.md` | Keep, possibly merge into Overview/Concepts. |
| `docs/readme.md:54 ### Job Types` | `docs/readme.md` | Keep as-is (table). |
| `docs/readme.md:66 ### Glossary` | `docs/readme.md` | Keep as-is (table). |
| `docs/readme.md:79 ## System Architecture` | `docs/readme.md` | Keep (as “Architecture”). |
| `docs/readme.md:81 ### System Context` | `docs/readme.md` | Keep mermaid (C4 L1). |
| `docs/readme.md:105 ### Container View (C4)` | `docs/readme.md` | Keep mermaid (C4 L2), but correct/standardize container list. |
| `docs/readme.md:160 ### Architecture Overview` | `docs/architecture/containers/dispatcher.md` | High-level internal flow diagram can live with orchestration/dispatcher narrative. |
| `docs/readme.md:254 ### Event Flow` | `docs/architecture/event_flow.md` | Move sequence diagram + short explanation. |
| `docs/readme.md:294 ### Component View: Orchestration` | `docs/architecture/containers/dispatcher.md` | Orchestration components are Dispatcher internals. |
| `docs/readme.md:349 ### Component View: Workers` | `docs/architecture/containers/workers.md` | Worker wrapper/operator contract diagram. |
| `docs/readme.md:386 ### Component View: Query Service` | `docs/architecture/containers/query_service.md` | Keep the diagram with Query Service doc. |
| `docs/readme.md:406 ## Core Components` | Split | Split content across dispatcher/workers/query_service + deploy docs. |
| `docs/readme.md:412 ### 1. Dispatcher` | `docs/architecture/containers/dispatcher.md` | Responsibilities, event model, routing, backpressure, failure mode. |
| `docs/readme.md:472 ### 2. SQS Queues` | `docs/architecture/containers/dispatcher.md` | Keep rationale here (optionally later extract to ADR). |
| `docs/readme.md:487 ### 3. Workers` | `docs/architecture/containers/workers.md` | Worker runtimes + execution model. |
| `docs/readme.md:508 ### Runtime Registry (Extensible)` | `docs/architecture/containers/dispatcher.md` | Keep full detail here; keep only a short mention in `docs/readme.md` if desired. |
| `docs/readme.md:523 ### 4. Postgres` | `docs/architecture/data_model/orchestration.md` | Tie “source of truth” narrative to schema reference. |
| `docs/readme.md:533 ### 5. Asset Storage` | `docs/architecture/containers/query_service.md` | Storage split + DuckDB federation detail lives with Query Service; keep short summary in `docs/readme.md`. |
| `docs/readme.md:555 ## Data Model` | `docs/readme.md` (doc map) | Replace this section with a doc map pointing at `docs/architecture/data_model/*`. |
| `docs/readme.md:572 ## Access Control` | `docs/standards/security_model.md` | Keep links from `docs/readme.md` to standards. |
| `docs/readme.md:590 ## PII and User Data` | `docs/architecture/data_model/pii.md` | Per decision. |
| `docs/readme.md:596 ## Job Lifecycle` | `docs/features/dag_configuration.md` | Lifecycle is primarily user-facing config semantics; also link to `docs/architecture/dag_deployment.md`. |
| `docs/readme.md:607 ## DAG Configuration` | `docs/features/dag_configuration.md` | Move to doc map and link. |
| `docs/readme.md:618 ## Infrastructure` | `docs/deploy/infrastructure.md` | Move. |
| `docs/readme.md:627 ## Deployment` | `docs/deploy/infrastructure.md` | Keep deployment order here or add `docs/deploy/README.md` later if it grows. |
| `docs/readme.md:637 ## Monitoring` | `docs/deploy/monitoring.md` | Move. |
| `docs/readme.md:649 ## Security` | `docs/standards/security_model.md` | Move; keep brief pointer from `docs/readme.md`. |
| `docs/readme.md:661 ## Appendix` / `docs/readme.md:663 ### References` | Distribute | Move Cryo links → ingestion/operator docs; DuckDB links → query_service; AWS links → deploy/infrastructure. |

