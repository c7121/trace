# User-Defined Functions (UDFs)

User-defined code for alerts, transforms, enrichments, and custom operators.

## Overview

UDFs allow users to define custom logic in their preferred runtime.

## Supported Runtimes

| Runtime | Use Case | Example |
|---------|----------|---------|
| TypeScript | JSON-heavy, async | `balance > 1000 ETH` |
| Python | ML, pandas, statistical | `df['value'].std() > threshold` |
| Rust | High-performance scanning | `col("value").gt(threshold)` |

## Contract

- **Input**: A pinned dataset version (or partition/range), plus config/parameters.
- **Output**: A result set (alerts, enriched rows, transformed partitions).
- **Stateless**: No state persists between invocations; all context is passed in.

## Sandbox

UDFs are **untrusted**. The full sandbox and isolation requirements live in
[security_model.md](../standards/security_model.md) (single source of truth).

In v1, assume:

- Runs in isolated execution environments with CPU/memory/timeout limits:
  - `ecs_udf` (ECS tasks) for long-running/heavy workloads
  - `lambda` (AWS Lambda) when required (short, event-driven tasks)
- No direct internet egress; UDFs can only call in-VPC platform services (e.g., Query Service, Dispatcher credential minting).
  - For Lambda, this means running inside a VPC without a NAT path (or enforcing egress at the VPC boundary).
- No direct Postgres access; ad-hoc reads go through the Query Service.
- No Secrets Manager access; secrets (when needed for trusted platform tasks) are injected at task launch.

UDFs should be deterministic for backfill/replay. Any non-deterministic values (e.g., time) must be passed explicitly as inputs/parameters.

### Data Access

UDFs do not receive broad infrastructure credentials. For task-scoped access they rely on:

- **Query Service** — `POST /v1/task/query` for SELECT-only SQL scoped to the task’s declared/pinned inputs. Large results may be exported to S3.
  See [query_service.md](../architecture/containers/query_service.md).
- **Dispatcher credential minting** — `POST /v1/task/credentials` to exchange the task capability token for short-lived STS credentials restricted to the task’s allowed S3 prefixes.
  See [dispatcher.md](../architecture/containers/dispatcher.md#credential-minting).

The capability token itself is defined in [contracts.md](../architecture/contracts.md#udf-data-access-token-capability-token).

## Use Cases


| Use Case | Description | Docs |
|---------|-------------|------|
| Alert conditions | Evaluate user-defined conditions on data | [alerting.md](alerting.md) |

Additional use cases (custom transforms, enrichments) are in the
[backlog](../plan/backlog.md#udf).

## Packaging

UDFs are submitted as code bundles (e.g., zip) and validated before execution.
See:

- [ADR 0003](../architecture/adr/0003-udf-bundles.md) for bundle format.
- [security_model.md](../standards/security_model.md) for signing/provenance requirements.
