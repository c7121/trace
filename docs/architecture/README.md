# Architecture index

Start here if you are implementing Trace.

## Canonical documents (read in this order)

1. **System invariants**: `invariants.md` (non-negotiable correctness + security truths)
2. **C4 overview**: `c4.md` (system/container diagrams)
3. **Task lifecycle**: `task_lifecycle.md` (leases, retries, outbox, idempotency)
4. **API and payload contracts**: `contracts.md` (task-scoped vs worker-only endpoints)
5. **User API contracts**: `user_api_contracts.md` (Gateway-exposed `/v1/*` routes and authz invariants)
6. **Data versioning**: `data_versioning.md` (dataset pointers, append/replace semantics, invalidations)
7. **Data model**: `data_model/` (DDL-level schemas)
8. **Containers**: `containers/` (service responsibilities and deployment units)
9. **Operators**: `operators/` (built-in job implementations and recipes)


## Doc ownership

See [docs_hygiene.md](../standards/docs_hygiene.md#doc-ownership) for the canonical ownership table.
