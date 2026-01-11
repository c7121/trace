# Cleanup Task 052: Tighten UDF specs and bundle manifest

## Goal

Make the UDF story coherent, safe-by-default, and low-drift by:
- clarifying doc ownership between the UDF model and the bundle manifest contract,
- resolving runtime and entrypoint terminology conflicts, and
- reducing repeated narrative while keeping safety constraints explicit and easy to find.

## Why

The current docs are close, but they contain a few drift and ambiguity risks:

- `docs/specs/udf.md` (UDF model) and `docs/specs/udf_bundle_manifest.md` (bundle manifest contract) both describe parts of the bundle/runtime model, which can drift.
- The meaning of "entrypoint" is inconsistent:
  - `docs/specs/udf.md` uses a Lambda-style handler string like `trace.handler`.
  - `docs/specs/udf_bundle_manifest.md` defines `entrypoint` as a relative file path.
- Runtime support is inconsistently described:
  - `docs/specs/udf.md` describes Node, Python, and Rust custom runtimes.
  - `docs/specs/udf_bundle_manifest.md` limits `runtime` to `node` or `python`.
  - ADR 0003 includes Rust bundle conventions.
- Several safety-critical constraints are repeated across multiple docs (UDF model, Lambda invocation contract, operator specs), increasing the chance of partial updates.

## Assessment summary (from review task 032)

### Proposed ownership boundary

- `docs/specs/udf.md` should own:
  - the UDF trust boundary model (untrusted user code),
  - what a UDF is allowed to do (task-scoped APIs only),
  - what UDFs are not allowed to do (no direct Postgres, no `/internal/*`, no arbitrary egress),
  - runtime modes at a conceptual level (lambda v1; ecs_udf reserved),
  - high-level language families, linked to ADR 0003.

- `docs/specs/udf_bundle_manifest.md` should own:
  - the exact `bundle_manifest.json` schema and versioning rules,
  - fail-closed validation requirements and caps (size, files, paths),
  - safe extraction and execution constraints (zip-slip, no symlinks, bounded stdout/stderr),
  - environment inheritance rules (deny by default; explicit allowlists only).

### Safety statements to centralize

Keep these statements explicit, but ensure each has one obvious owner and the other docs link to it:

- UDF code is untrusted in all runtimes and must authenticate only with a per-attempt capability token over TLS.
- UDFs must not call `/internal/*` endpoints and must not have direct Postgres credentials.
- No third-party internet egress by default for untrusted runtimes (pair with Query Service SQL gating and runtime sandboxing).
- Bundles are immutable and pinned by content hash; runners must verify bundle integrity before executing.
- Bundle extraction must be fail-closed with strict path safety rules (no traversal, no absolute paths, no backslashes, no symlinks).

### Key ambiguity to fix

The meaning of "entrypoint" needs one consistent contract across:
- the DAG `udf` block,
- the bundle manifest, and
- the ADR narrative.

If the platform intends Lambda-style handlers for Node/Python, the bundle manifest should use the same handler concept (and reserve file-path-only entry execution for future if needed).

## Plan

- Tighten `docs/specs/udf.md` (link-first, no duplication):
  - Link to `docs/architecture/contracts/lambda_invocation.md` for the invocation payload and lease fencing details instead of restating them.
  - Link to `docs/architecture/contracts/task_capability_tokens.md` and `docs/architecture/contracts/task_scoped_endpoints.md` for token and endpoint details.
  - Reduce repeated bundle packaging details by linking to ADR 0003 and `docs/specs/udf_bundle_manifest.md`.
  - Make language and runtime support explicit with clear status (v1 supported vs planned).
- Tighten `docs/specs/udf_bundle_manifest.md` (schema-first, fail-closed):
  - Resolve the "entrypoint" terminology conflict by aligning the manifest field with the chosen Node/Python handler contract.
  - Align the `runtime` enum with the intended language support, or explicitly mark unsupported runtimes as rejected for v1.
  - Clarify `env_allowlist` as a request that is intersected with a runner-owned allowlist (never user-controlled pass-through of sensitive env vars).
  - Ensure versioning and forward-compat behavior is explicit (unknown `schema_version` is rejected).
- Keep operator docs small:
  - `docs/specs/operators/udf.md` should remain a thin operator surface and link to `docs/specs/udf.md` for the trust boundary story.

## Files to touch

- `docs/specs/udf.md`
- `docs/specs/udf_bundle_manifest.md`
- `docs/specs/operators/udf.md` (link-first tightening only)

## Acceptance criteria

- A reader can answer "what is a UDF", "what can it do", and "how is it constrained" with minimal duplication and clear links to contract owners.
- The meaning of `entrypoint` is consistent across UDF docs and does not conflict with Lambda handler conventions.
- Runtime and language support is explicit and consistent across UDF docs and ADR references.
- Bundle manifest validation requirements are fail-closed and unambiguous, including env inheritance rules.

## Suggested commit message

`docs: tighten udf specs`
