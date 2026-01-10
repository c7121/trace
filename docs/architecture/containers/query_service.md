# Query Service

Stateless service that executes validated, read-only SQL against authorized hot and cold storage.

## Overview

| Property | Value |
|----------|-------|
| **Type** | Platform service |
| **Runtime** | Rust + embedded DuckDB |
| **Deployment** | ECS Fargate behind an internal ALB |

## Doc ownership

This doc is intentionally link-first. API semantics and data model details live elsewhere:

- API semantics:
  - User query endpoint: `docs/specs/query_service_user_query.md`
  - Task query endpoint: `docs/specs/query_service_task_query.md`
- Query results and exports: `docs/specs/query_service_query_results.md`
- SQL gate: `docs/specs/query_sql_gating.md`
- Capability token rules: `docs/architecture/contracts.md`
- Operations defaults: `docs/architecture/operations.md`
- Security invariants: `docs/architecture/security.md`
- Data model: `docs/architecture/data_model/query_service.md`
- PII audit rules: `docs/architecture/data_model/pii.md`
- Query results decision: `docs/adr/0005-query-results.md`

## Architecture

```mermaid
flowchart LR
    user["User"] -->|/v1/query| gateway["Gateway"]:::container
    task["Lambda UDF runner - untrusted"] -->|/v1/task/query| qs["Query Service"]:::container
    gateway -->|/v1/query| qs

    qs -->|read| pg["Postgres data"]:::database
    qs -->|read parquet| obj["Object store"]:::database

    classDef container fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
```

## Responsibilities and invariants

- Authenticate and authorize the caller:
  - `/v1/query` uses a user Bearer JWT.
  - `/v1/task/query` uses a task capability token.
- Enforce read-only, fail-closed SQL execution:
  - Always gate with `trace-core::query::validate_sql`.
  - Apply DuckDB runtime hardening as defense-in-depth.
- Attach datasets in trusted code only:
  - Expose authorized datasets as stable relations (for example `dataset`).
  - Execute untrusted SQL only against attached relations.
- Enforce data access boundaries:
  - No arbitrary filesystem access from untrusted SQL.
  - When remote Parquet scans are supported, restrict egress to the configured object store endpoints.
- Emit audit records without storing raw SQL and follow PII audit rules.

## Interfaces

- `POST /v1/query` - user-scoped interactive query: `docs/specs/query_service_user_query.md`
- `POST /v1/task/query` - task-scoped query: `docs/specs/query_service_task_query.md`

## Query capabilities

Query Service supports a constrained SQL surface and is designed to fail closed. The canonical allow and deny rules live in `docs/specs/query_sql_gating.md`.

## Query results

Query executions that produce persisted results use the platform-managed `query_results` table. See `docs/adr/0005-query-results.md` and `docs/architecture/data_model/query_service.md`.
