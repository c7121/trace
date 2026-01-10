# Trace docs

This directory contains the design and architecture documentation for Trace.

For a product overview and user stories, see the repo root [README.md](../README.md).

## Start here

If you are implementing Trace:
- Architecture index: [architecture/README.md](architecture/README.md)
- System invariants: [architecture/invariants.md](architecture/invariants.md)
- Security model: [architecture/security.md](architecture/security.md)
- Operations: [architecture/operations.md](architecture/operations.md)
- C4 diagrams: [architecture/c4.md](architecture/c4.md)
- Contracts: [architecture/contracts.md](architecture/contracts.md)

If you are designing or changing behavior:
- Specs: [specs/README.md](specs/README.md)
- ADRs: [adr/](adr/)

If you are operating or deploying:
- Deploy: [deploy/](deploy/)
- Examples: [examples/](examples/)

If you are working on operators:
- Operator specs: [specs/operators/README.md](specs/operators/README.md)

If you are using Trace Lite:
- Harness: [harness/README.md](../harness/README.md)

## Workflow

- Design: start with the relevant spec in [specs/](specs/), then confirm [architecture/invariants.md](architecture/invariants.md), [architecture/contracts.md](architecture/contracts.md), and [architecture/security.md](architecture/security.md).
- Implement: use the relevant container doc in [architecture/containers/](architecture/containers/) and operator spec in [specs/operators/README.md](specs/operators/README.md). Update [architecture/data_model/](architecture/data_model/) if schemas change.
- Validate: use [examples/](examples/) for end-to-end runs and keep the harness green (see [harness/README.md](../harness/README.md)).

Project sequencing:
- Planning: [plan/README.md](plan/README.md)
