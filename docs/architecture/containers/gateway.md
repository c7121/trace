# Gateway

Entry point for all external API traffic. Routes requests to backend services.

## Overview

| Property | Value |
|----------|-------|
| **Type** | API Gateway + internal ALB |
| **Deployment** | API Gateway (JWT) with VPC Link to internal ALB |
| **Auth** | Bearer token (JWT from IdP) |

## Architecture

```mermaid
flowchart LR
    users["Users"]
    apigw["API Gateway"]
    alb["ALB"]
    
    subgraph Backend["Backend Services"]
        dispatcher["Dispatcher"]
        query["Query Service"]
    end
    
    idp["IdP Cognito"]
    
    users -->|/v1/query| apigw
    users -->|/v1/jobs, /v1/alerts, etc.| apigw
    apigw -->|validate token| idp
    apigw -->|route| alb
    alb --> dispatcher
    alb --> query
```

## Endpoints

| Path | Backend | Purpose |
|------|---------|---------|
| `POST /v1/query` | Query Service | Interactive SQL queries |
| `GET /v1/jobs/*` | Dispatcher | Job status, list |
| `POST /v1/emit` | Dispatcher | Manual event emission |
| `POST /v1/backfill` | Dispatcher | Trigger backfill |
| `GET /v1/tasks/*` | Dispatcher | Task status |
| `GET,POST,PUT,DELETE /v1/alerts/*` | Dispatcher | Alert CRUD |
| `GET,POST,PUT,DELETE /v1/labels/*` | Dispatcher | Address label CRUD |
| `GET,POST,PUT,DELETE /v1/queries/*` | Dispatcher | Saved query CRUD |
| `POST /v1/dags` | Dispatcher | DAG deployment |

## Authentication

All requests require `Authorization: Bearer <token>`.

1. API Gateway validates JWT signature against IdP JWKS
2. Extracts claims: `sub`, `org_id`, `email`, `role`
3. Passes claims to backend via headers:
   - `X-Org-Id`: org UUID
   - `X-User-Id`: user UUID (resolved from `sub`)
   - `X-User-Role`: platform role (reader/writer/admin)
4. Backend services may use these forwarded headers as **convenience**, but they must treat the **Bearer JWT** (validated by API Gateway and re-validated or verified by the backend) as the source of truth for identity/role. Do **not** treat forwarded identity headers as a standalone trust boundary, because backend services are also reachable from inside the VPC (workers, Lambdas).

> **Hard requirement:** do not expose the ALB to the public internet. If the ALB must be internet-facing, backend services must validate JWTs themselves (and apply rate limits/WAF).


> **Not exposed:** task-scoped endpoints like `/v1/task/query` and `/v1/task/credentials` are **internal-only** and must not be routed through the public Gateway. They are called by workers/Lambdas using a task capability token (see `security_model.md`).

## Rate Limiting

| Scope | Limit | Window |
|-------|-------|--------|
| Per-org | 1000 req | 1 minute |
| Per-user | 100 req | 1 minute |
| Query endpoint | 10 req | 1 minute |

Enforced at API Gateway layer. Returns `429 Too Many Requests` when exceeded.

## CORS

Allowed origins configured per environment. Default: same-origin only.

## Related

- [query_service.md](query_service.md) — query endpoint details
- [contracts.md](../contracts.md) — dispatcher API contracts
- [security_model.md](../../standards/security_model.md) — auth model
