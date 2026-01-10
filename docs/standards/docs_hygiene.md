# Docs Hygiene Checklist

This repo is documentation-heavy by design. The goal is **clarity with minimal surface area**.

## Terminology

Use these canonical names consistently:

- **Postgres state**: control-plane database (jobs, tasks, leases, outbox)
- **Postgres data**: data-plane database (hot chain tables, alert events, query results, buffered datasets)
- **SQS task queues**: runtime wake-up queues for tasks (at-least-once, unordered)
- **SQS dataset buffers**: queues used by buffered Postgres datasets (ADR 0006)
- **Query Service**: the only Postgres read path for untrusted UDFs
- **Dispatcher credential minting**: mints short-lived S3 credentials scoped to a task capability token
- **RPC Egress Gateway**: the only outbound RPC path for jobs that need chain RPC access

> **Rule of thumb:** job containers have no direct internet egress. Only platform egress services (Delivery Service, RPC Egress Gateway) can reach the public internet.
- **Delivery Service**: outbound internet egress for alerting (webhooks, Slack, etc.)

## Mermaid diagrams

Some Mermaid renderers are strict. Keep diagrams robust:

- Avoid parentheses in **labels**.
- Avoid bracket characters `[` and `]` inside labels.
- Keep edge labels short and ASCII.
- If a diagram fails to render, simplify labels/punctuation before changing the structure.

## Reduce duplication

Prefer linking to the canonical pages instead of re-explaining:

- C4 diagrams: [c4.md](../architecture/c4.md)
- Task lifecycle + durability: [task_lifecycle.md](../architecture/task_lifecycle.md)
- Data versioning + commit protocol: [data_versioning.md](../architecture/data_versioning.md)

## Doc ownership

If you need to explain the same thing twice, pick one owner and link to it.

| Concept | Owner |
|---------|-------|
| Correctness under failure (retries, leases, outbox) | [task_lifecycle.md](../architecture/task_lifecycle.md) |
| Wire-level contracts (payloads, auth, fencing) | [contracts.md](../architecture/contracts.md) |
| User-facing routes and authz | [user_api_contracts.md](../architecture/user_api_contracts.md) |
| Trust boundaries and identity | [security_model.md](security_model.md) |
| Product/feature intent | specs/ (should not duplicate wire contracts) |
| Decision rationale | adr/ |

## Links

- Do not add links to files that do not exist.
- Prefer relative links inside `docs/`.

## ADRs

- ADRs should state the v1 decision. Avoid leaving “Open Questions” once a decision is made.
