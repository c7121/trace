# Architecture index

Start here if you are implementing Trace.

## Canonical documents (read in this order)

1. **C4 overview**: `c4.md` (system/container diagrams)
2. **Task lifecycle**: `task_lifecycle.md` (leases, retries, outbox, idempotency)
3. **API and payload contracts**: `contracts.md` (task-scoped vs worker-only endpoints)
4. **Data versioning**: `data_versioning.md` (dataset pointers, append/replace semantics, invalidations)
5. **Data model**: `data_model/` (DDL-level schemas)
6. **Containers**: `containers/` (service responsibilities and deployment units)
7. **Operators**: `operators/` (built-in job implementations and recipes)

## Doc ownership rules (keep this repo coherent)

- **`task_lifecycle.md`** owns correctness under failure (retries, leases, outbox).
- **`contracts.md`** owns wire-level contracts (payload fields, required auth, fencing).
- **`standards/security_model.md`** owns trust boundaries and identity contexts.
- **Specs (`docs/specs/`)** own product/feature intent and public surface; they SHOULD NOT duplicate wire contracts.
- **ADRs (`docs/adr/`)** own the “why” and scope-limiting decisions.

If you need to explain the same thing twice, pick one owner doc and link to it.
