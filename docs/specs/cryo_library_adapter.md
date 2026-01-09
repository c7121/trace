# Cryo library adapter and artifact sink spike

Status: Draft
Owner: Platform
Last updated: 2026-01-09

## Summary
Today the Cryo worker shells out to the Cryo CLI, writes Parquet to a local staging directory, then uploads to object storage.
This spike proposes embedding Cryo as a Rust library in the worker and introducing a writer abstraction so Parquet can be streamed directly to the configured object store without local staging.

## Risk
Medium

## Related ADRs
None

## Context
- Lite mode is acceptable with local staging, but it increases disk footprint and cleanup risk for large ranges.
- The worker already uploads Parquet without reading whole files into RAM (`trace_core::ObjectStore::put_file`), but local staging is still required because Cryo writes to `output_dir`.
- Cryo also produces internal reports under `output_dir/.cryo/`; Trace should not require those or depend on their format.

## Goals
- Identify a safe integration boundary to run Cryo in-process while preserving existing task semantics (at-least-once, idempotent publication).
- Make it feasible to stream Parquet artifacts directly to object storage (S3/MinIO) without local staging in the primary path.
- Keep any Trace-owned "manifest" concept separate from Cryo outputs (no Trace requirements on Cryo artifacts).

## Non-goals
- Implement this change in the worker now.
- Redesign Cryo dataset schemas or require a relational schema for chain datasets.
- Add new user-facing API surface.

## Public surface changes
None (spec-only).

## Proposed design

### Survey of Cryo code points (investigation checklist)
In the Cryo repo, identify:
- Where CLI args are parsed and the dataset runner is invoked.
- Where `output_dir` is validated and per-dataset output paths are constructed.
- Where Parquet writers are created and write to file paths.
- Where the list of produced Parquet files is discovered (for publication) and what metadata is already available.

### Adapter boundary
Introduce a Trace-owned writer interface that Cryo can call instead of writing to local file paths:

- `trait ArtifactSink { fn put_object(...) -> ... }`
  - Minimal methods needed for Parquet and small sidecars.
  - Prefer streaming inputs (reader/stream) over `Vec<u8>`.
  - Include size caps and deterministic key derivation at the boundary.

Trace-owned publication remains unchanged:
- Worker completes the task by publishing exactly one `DatasetPublication` per attempt.
- Dispatcher remains the only component that registers dataset versions in Postgres state.

### Migration plan
- Phase 0 (current): shell out to Cryo CLI and upload staged Parquet to object store.
- Phase 1: embed Cryo as a library behind a feature flag; keep staging as fallback.
- Phase 2: implement `ArtifactSink` and remove staging from the default path.
- Phase 3: upstream Cryo support (optional): native sink hooks or direct object store output.

## Contract requirements
- The embedded Cryo path MUST preserve at-least-once semantics and idempotent dataset publication.
- The worker MUST enforce artifact caps (count and bytes) before writing or uploading.
- The worker MUST delete any temporary state on success and attempt crash cleanup on startup.
- Trace MUST NOT require any Cryo report files under `.cryo/` for correctness.

## Security considerations
- Streaming to object storage reduces local disk exposure, but the worker still handles untrusted RPC data.
- The worker should treat all Cryo outputs as untrusted until published and queried through Query Service, which remains fail-closed.

## Reduction pass
- Avoids expanding surface area by keeping the publication contract unchanged (still `/v1/task/complete` with `DatasetPublication`).
- Avoids new modes by making staging a fallback only during transition.

## Acceptance criteria
- A short design review confirms the injection points in Cryo and the minimal `ArtifactSink` API.
- A POC can run one dataset range in-process and publish a dataset version without writing Parquet to local disk in the primary path.
