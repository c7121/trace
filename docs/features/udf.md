# User-Defined Functions (UDFs)

User-defined code for alerts, transforms, enrichments, and custom operators.

## Overview

UDFs allow users to define custom logic in their preferred runtime. All UDFs share a common sandbox and contract.

## Supported Runtimes

| Runtime | Use Case | Example |
|---------|----------|---------|
| TypeScript | JSON-heavy, async | `balance > 1000 ETH` |
| Python | ML, pandas, statistical | `df['value'].std() > threshold` |
| Rust | High-performance scanning | `col("value").gt(threshold)` |

## Contract

- **Input**: Data partition or row set, plus config/parameters.
- **Output**: Result set (e.g., triggered alerts, enriched rows, transformed data).
- **Stateless**: No state persists between invocations; all context passed in.

## Sandbox

User-defined code runs in isolated containers with strict constraints:

- **Isolation**: Each invocation runs in its own container (see [security_model.md](../standards/security_model.md) for container/network isolation).
- **Resource caps**: CPU (0.25 vCPU default), memory (512 MB default), timeout (60s default); configurable per job.
- **No network**: UDFs cannot make outbound calls; data is injected, results returned.
- **No platform control-plane access**: UDFs cannot call Trace internal APIs (e.g., `/internal/*`); only the worker wrapper/runtime may interact with platform endpoints.
- **No filesystem**: Read-only except for ephemeral `/tmp`; no persistent state.
- **Determinism**: UDFs must be deterministic—same inputs produce same outputs. Required for backfill/replay consistency. Non-deterministic functions (e.g., `random()`, `now()`) are prohibited or injected as parameters.
- **Allowed imports**: Restricted to a vetted set of libraries per runtime; no arbitrary package installation.

## Use Cases

| Use Case | Description | Docs |
|----------|-------------|------|
| Alert conditions | Evaluate user-defined conditions on data | [alerting.md](alerting.md) |

Additional use cases (custom transforms, enrichments) are in the [backlog](../plan/backlog.md#udf).

## Packaging

UDFs are submitted as code bundles (e.g., zip) and validated before execution. Bundles are versioned and signed; see [security_model.md](../standards/security_model.md) for signing requirements.

v1 uses AWS Lambda-style zip bundles (including Rust custom runtime `bootstrap`) executed in ECS for maximum tooling reuse. See [ADR 0003](../architecture/adr/0003-udf-bundles.md).
