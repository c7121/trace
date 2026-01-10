# Architecture index

Start here if you are implementing Trace.

## Canonical documents (read in this order)

1. **System invariants**: `invariants.md` (non-negotiable correctness + security truths)
2. **Security model**: `security.md` (trust boundaries, auth model, enforceable invariants)
3. **C4 overview**: `c4.md` (system/container diagrams)
4. **Task lifecycle**: `task_lifecycle.md` (leases, retries, outbox, idempotency)
5. **API and payload contracts**: `contracts.md` (task-scoped vs worker-only endpoints)
6. **User API contracts**: `user_api_contracts.md` (Gateway-exposed `/v1/*` routes and authz invariants)
7. **Operations**: `operations.md` (operational targets, defaults, and runbooks)
8. **Data versioning**: `data_versioning.md` (dataset pointers, append/replace semantics, invalidations)
9. **Data model**: `data_model/` (DDL-level schemas)
10. **Containers**: `containers/` (service responsibilities and deployment units)
11. **Operators**: `operators/` (built-in job implementations and recipes)


## Doc ownership

| Concept | Owner |
|---------|-------|
| Correctness under failure (retries, leases, outbox) | [task_lifecycle.md](task_lifecycle.md) |
| Wire-level contracts (payloads, auth, fencing) | [contracts.md](contracts.md) |
| User-facing routes and authz | [user_api_contracts.md](user_api_contracts.md) |
| Trust boundaries and identity | [security.md](security.md) |
| Product and feature intent | [specs/](../specs/) |
| Decision rationale | [adr/](../adr/) |
