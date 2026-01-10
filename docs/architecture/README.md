# Architecture index

Start here if you are implementing Trace.

## Core concepts

These are the nouns used throughout the architecture docs:

- **DAG**: versioned YAML definition (jobs + edges + publish). Canonical schema: [docs/specs/dag_configuration.md](../specs/dag_configuration.md).
- **Job**: a named node in a DAG (operator + runtime + config) that produces one or more outputs.
- **Task**: one execution of a job for a specific input update or partition, created and leased by the Dispatcher.
- **Attempt**: a retry counter for a task. Only the current attempt may heartbeat or complete.
- **Lease**: a time-bounded right to execute the current attempt (`lease_token`, `lease_expires_at`).
- **Outbox**: a durable record of a side effect (enqueue, route) written in the same transaction as orchestration state.
- **Dataset identity**: `{dataset_uuid, dataset_version}` identifies the data version used for routing, pinning, and lineage.

## Canonical documents (read in this order)

After reading Core concepts above:

1. **System invariants**: [invariants.md](invariants.md) (non-negotiable correctness + security truths)
2. **Security model**: [security.md](security.md) (trust boundaries, auth model, enforceable invariants)
3. **C4 overview**: [c4.md](c4.md) (system/container diagrams)
4. **Task lifecycle**: [task_lifecycle.md](task_lifecycle.md) (leases, retries, outbox, idempotency)
5. **API and payload contracts**: [contracts.md](contracts.md) (task-scoped vs worker-only endpoints)
6. **User API contracts**: [user_api_contracts.md](user_api_contracts.md) (Gateway-exposed `/v1/*` routes and authz invariants)
7. **Operations**: [operations.md](operations.md) (operational targets, defaults, and examples)
8. **Data versioning**: [data_versioning.md](data_versioning.md) (behavior and invariants; schema mapping in [data_model/data_versioning.md](data_model/data_versioning.md))
9. **Data model**: [data_model/](data_model/) (DDL-level schemas)
10. **Containers**: [containers/](containers/) (service responsibilities and deployment units)
11. **Operators**: [Operator specs](../specs/operators/README.md) (built-in operator surfaces and status)


## Doc ownership

| Concept | Owner |
|---------|-------|
| Correctness under failure (retries, leases, outbox) | [task_lifecycle.md](task_lifecycle.md) |
| Wire-level contracts (payloads, auth, fencing) | [contracts.md](contracts.md) |
| User-facing routes and authz | [user_api_contracts.md](user_api_contracts.md) |
| Trust boundaries and identity | [security.md](security.md) |
| Product and feature intent | [specs/](../specs/) |
| Decision rationale | [adr/README.md](../adr/README.md) |
