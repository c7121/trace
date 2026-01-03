# udf

Generic UDF execution harness.

This operator runs an untrusted user bundle and provides access only to task-scoped APIs authenticated by the per-attempt **task capability token**:
- Query Service reads (`/v1/task/query`)
- Dispatcher fenced calls (`/v1/task/heartbeat`, `/v1/task/complete`, `/v1/task/events`, `/v1/task/buffer-publish`)
- Scoped object-store credentials minted per task (`/v1/task/credentials`)

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
- UDFs are untrusted; they must not call `/internal/*` endpoints.
