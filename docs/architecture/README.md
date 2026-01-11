# Architecture index

Start here if you are implementing Trace.

## Core concepts

These are the nouns used throughout the architecture docs. Each item includes a best next hop link.

- **DAG**: versioned YAML definition (jobs + edges + publish). Best next hop: [dag_configuration.md](../specs/dag_configuration.md).
- **Job**: a named node in a DAG (operator + runtime + config) that produces one or more outputs. Best next hop: [dag_configuration.md](../specs/dag_configuration.md).
- **Operator**: the stable "what code runs" surface for a job: operator name, config semantics, and input/output expectations. Best next hop: [Operator specs](../specs/operators/README.md).
- **Task**: one execution of a job for a specific input update or partition, created and leased by the Dispatcher. Best next hop: [task_lifecycle.md](task_lifecycle.md).
- **Attempt**: a retry counter for a task. Only the current attempt may heartbeat or complete. Best next hop: [contracts/task_scoped_endpoints.md](contracts/task_scoped_endpoints.md).
- **Lease**: a time-bounded right to execute the current attempt (`lease_token`, `lease_expires_at`). Best next hop: [contracts/task_scoped_endpoints.md](contracts/task_scoped_endpoints.md).
- **Outbox**: a durable record of a side effect written in the same transaction as orchestration state (enqueue, route). Best next hop: [task_lifecycle.md](task_lifecycle.md).
- **Dataset**: a stable dataset identified by `dataset_uuid` with versioned generations identified by `dataset_version`; tasks and queries pin to a generation for routing and lineage, and "current" pointers live in Postgres state. Best next hop: [data_versioning.md](data_versioning.md).

## Canonical documents (read in this order)

After reading Core concepts above:

1. **System invariants**: [invariants.md](invariants.md) (non-negotiable correctness + security truths)
2. **Security model**: [security.md](security.md) (trust boundaries, auth model, enforceable invariants)
3. **C4 overview**: [c4.md](c4.md) (system/container diagrams)
4. **Database boundaries**: [db_boundaries.md](db_boundaries.md) (state vs data ownership, cross-DB identifiers)
5. **Task lifecycle**: [task_lifecycle.md](task_lifecycle.md) (leases, retries, outbox, idempotency)
6. **API and payload contracts**: [contracts.md](contracts.md) (task-scoped vs worker-only endpoints)
7. **User API contracts**: [user_api_contracts.md](user_api_contracts.md) (Gateway-exposed `/v1/*` routes and authz invariants)
8. **Operations**: [operations.md](operations.md) (operational targets, defaults, and examples)
9. **Data versioning**: [data_versioning.md](data_versioning.md) (behavior and invariants; schema mapping in [data_model/data_versioning.md](data_model/data_versioning.md))
10. **Data model**: [data_model/README.md](data_model/README.md) (DDL-level schemas)
11. **Containers**: [containers/README.md](containers/README.md) (service responsibilities and deployment units)
12. **Operators**: [Operator specs](../specs/operators/README.md) (built-in operator surfaces and status)


## Doc ownership

| Concept | Owner |
|---------|-------|
| Correctness under failure (retries, leases, outbox) | [task_lifecycle.md](task_lifecycle.md) |
| Wire-level contracts (payloads, auth, fencing) | [contracts.md](contracts.md) |
| User-facing routes and authz | [user_api_contracts.md](user_api_contracts.md) |
| Trust boundaries and identity | [security.md](security.md) |
| Product and feature intent | [../specs/README.md](../specs/README.md) |
| Decision rationale | [adr/README.md](../adr/README.md) |
