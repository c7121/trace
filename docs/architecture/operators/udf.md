# udf

Generic UDF execution harness.

This operator runs an untrusted user bundle and provides access only to task-scoped APIs:
- Query Service reads (capability token)
- Dispatcher task-plane calls (worker token): heartbeat/complete/buffer publish
- Scoped object-store credentials minted per task (capability token)

It intentionally has no built-in domain logic. Domain-specific patterns (like alert evaluation) can be implemented as dedicated operators or as UDF code.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `lambda` (v1) |
| **Activation** | `reactive` |
| **Execution Strategy** | PerUpdate or PerPartition |
| **Idle Timeout** | `5m` |

## Inputs and outputs

Defined entirely by the DAG wiring and the UDF bundle logic.

## Configuration

UDF jobs MUST include an `udf` block in DAG config:

```yaml
udf:
  bundle_id: "<bundle-id>"
  entrypoint: "trace.handler"
```

## Notes

- UDF jobs MUST NOT request `secrets`.
- See `docs/specs/udf.md` and ADR 0003 for bundle format and provenance.
