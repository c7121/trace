# ADR 0003: UDF Bundles (Lambda-Style Packaging)

## Status
- Accepted (December 2025)

## Decision
- User-defined code (alert conditions, transforms, enrichments) is packaged and distributed as **AWS Lambda-style zip bundles** stored in S3.
- v1 standardizes execution on **`linux/amd64`** so users build a single artifact target.
- Bundles are executed by Trace workers (ECS) via the worker wrapper:
  - Wrapper downloads the bundle from S3 (scoped read access), verifies integrity, and runs it in an isolated container.
  - Wrapper provides a **Lambda Runtime API-compatible invocation loop** (single-invocation per task) so standard Lambda runtime libraries can run unmodified.

### Bundle Formats

- **Rust (custom runtime)**: zip contains a `bootstrap` executable at the archive root (Lambda custom runtime convention).
- **Node (Lambda-style handler)**: zip contains handler code (and optional dependencies) using common Lambda packaging patterns (e.g., Serverless/SAM/esbuild outputs).

## Context
- GTM requires a clean path for users to run custom logic (Rust/Polars and Node/ethers) without compiling inside the platform.
- Reusing existing AWS Lambda bundling tooling improves developer ergonomics and reduces bespoke packaging work.
- User jobs run with **no internet egress by default** and must access data only through platform primitives:
  - **Query Service** for ad-hoc SQL reads (no direct Postgres access for UDFs)
  - **Credential Broker** for short-lived, prefix-scoped S3 credentials (no broad IAM in UDF tasks)
  (see ADR 0002 and [security_model.md](../../standards/security_model.md)).

## Why
- **Tooling reuse**: users can leverage `cargo-lambda`, SAM, Serverless, and common build pipelines that already output Lambda-compatible zips.
- **Operational simplicity**: the platform executes prebuilt artifacts; no in-cluster compilation, no dynamic dependency installs.
- **Security**: bundles are immutable artifacts, suitable for signing and verification before execution.
- **Portability**: Lambda-compatible zips are a well-understood “function bundle” interchange format.

## Consequences
- The worker wrapper must implement enough of the Lambda Runtime API to support common runtimes (fetch next invocation, post response/error).
- v1 requires `linux/amd64` artifacts for any native bundles (e.g., Rust `bootstrap`).
- Node bundles must be deterministic and run without outbound network access; `ethers` usage is for decoding/formatting over task-provided data.
- The task payload must fully describe allowed inputs/outputs so the wrapper can scope data access:
  - Query Service attaches only dataset views enumerated in the task capability token.
  - Credential Broker issues S3 credentials scoped to the task’s allowed prefixes.

## Trade-offs
- Added wrapper complexity versus a bespoke “stdin/stdout” contract.
- Lambda compatibility constrains the invocation model; multi-invocation “warm loops” are possible but deferred (v1 uses one invocation per task).

## Open Questions
- Bundle signing format and verification policy (cosign vs. detached signatures vs. hash pinning in DB).
- Node dependency model for GTM (deps baked into runtime image vs vendored per-bundle).
- Future support for `linux/arm64` as an additional runtime (e.g., `ecs_rust_arm64`, `ecs_node_arm64`).
