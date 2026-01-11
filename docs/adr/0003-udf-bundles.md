# ADR 0003: UDF Bundles (Lambda-Style Packaging)

## Status
- Accepted (December 2025)

### Amendment (January 2026)
- v1 executes untrusted UDF bundles only via the **platform-managed Lambda UDF runner** (`runtime: lambda`).
- **ECS UDF execution is deferred to v2** because ECS/Fargate does not support per-container IAM roles; achieving zero trust requires a different launcher/credential isolation design.
- v1 UDF bundles are limited to Node and Python today. Rust custom runtime bundles are planned as part of milestone 18 (see `docs/plan/milestones.md`).

## Decision
- User-defined code (alerts, custom transforms/enrichments) is packaged and distributed as **AWS Lambda-style zip bundles** stored in object storage.
- v1 standardizes native artifacts on **`linux/amd64`** (so users build a single target for Rust/custom runtimes and any native extensions).
- The platform records **bundle metadata** at upload time (at minimum: `sha256`, `language`, and an optional default `entrypoint`).
- At execution time, the Dispatcher selects an appropriate **language runner** based on the bundle metadata.

Bundles are executed by Trace runtimes as follows:
- **v1:** `runtime: lambda` via platform-managed language runners (Node, Python).
- **v2 (deferred):** `ecs_udf` once a zero-trust launcher/credential isolation design exists.

## Bundle formats

UDF bundles are immutable zip artifacts.

- **Node.js (JavaScript/TypeScript)**
  - Bundle contains JS handler code (and vendored dependencies).
  - TypeScript is compiled to JavaScript and shipped as a Node bundle.
- **Python**
  - Bundle contains Python handler code (and vendored dependencies).
- **Rust (Lambda custom runtime)** (planned)
  - Bundle contains a `bootstrap` executable at the archive root (Lambda custom runtime convention).
  - Recommended tooling: `cargo-lambda` (or equivalent) to produce the zip; the platform expects the standard AWS custom runtime layout.

## Context
- We want a clean path for users to run custom logic without compiling inside the platform.
- Reusing existing AWS Lambda bundling tooling improves developer ergonomics and reduces bespoke packaging work.
- User jobs run with **no third-party internet egress by default** and must access data only through platform primitives:
  - Query Service for task-scoped reads (no direct Postgres access for UDFs)
  - Dispatcher credential minting for short-lived, prefix-scoped object-store credentials

## Why
- Tooling reuse: users can leverage `cargo-lambda`, SAM, Serverless, `pip` vendoring, etc.
- Operational simplicity: the platform executes prebuilt artifacts; no in-cluster compilation or dependency installs.
- Security: bundles are immutable artifacts that can be pinned/verified.
- Portability: a Lambda-style zip is a well-understood “function bundle” interchange format.

## Consequences
- UDFs are executed as **untrusted** code.
  - The platform passes only a per-attempt **task capability token**; do not inject long-lived internal secrets into bundles.
- Language runners are platform-managed and must not become a privileged escape hatch:
  - The runner’s IAM role should be near-zero.
  - The Dispatcher should pass a pre-signed URL for the bundle, so the runner does not need broad S3 access.
- A single DAG may mix languages by referencing different bundles in different jobs.

AWS ECS note:
- ECS/Fargate does not support per-container IAM roles. If a privileged wrapper and an untrusted UDF share an ECS task, the UDF inherits the wrapper’s permissions.
- To maintain zero trust, `ecs_udf` must prevent untrusted code from inheriting privileged AWS permissions.

## Trade-offs
- Added runner complexity versus a bespoke “stdin/stdout” contract.
- Lambda compatibility constrains the invocation model; multi-invocation “warm loops” are possible but deferred (v1 uses one invocation per task).

## v1 policy

- **Integrity:** bundles are pinned by SHA-256 in the control plane and MUST be verified before execution.
- **Signing:** bundle signing/verification is deferred.
- **Dependencies:** dependencies MUST be vendored in the bundle (no `npm install` / `pip install` at runtime; no outbound network required).
- **Architectures:** v1 supports `linux/amd64` only.

## Related

- Normative surface: [udf.md](../specs/udf.md) and [udf_bundle_manifest.md](../specs/udf_bundle_manifest.md)
- UDF operator surface: [operators/udf.md](../specs/operators/udf.md)
- Lambda invocation contract: [lambda_invocation.md](../architecture/contracts/lambda_invocation.md)
- Task capability tokens: [task_capability_tokens.md](../architecture/contracts/task_capability_tokens.md)
- Milestone tracking: [milestones.md](../plan/milestones.md)
