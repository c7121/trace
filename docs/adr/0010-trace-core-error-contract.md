# ADR 0010: trace-core error contract

Date: 2026-01-11
Status: Accepted

## Context
`trace-core` is an internal crate (`publish = false`) that defines cross-crate interfaces for Trace Lite (queues, object storage, signing, and runtime invocation).

Exposing `anyhow::Result` in its public API leaks an implementation detail and makes it harder to keep error handling consistent across crates.

We want:
- A crate-owned error type for all public `trace-core` traits and helpers.
- `anyhow` available internally for context, without being part of the public API surface.

## Decision
- `trace-core` defines `trace_core::Error` and `trace_core::Result<T>`.
- All public `trace-core` traits return `trace_core::Result<T>` (not `anyhow::Result`).
- `trace_core::Error` is a thin newtype wrapper over `anyhow::Error` so:
  - implementations can keep using `anyhow` internally with context strings, and
  - callers using `anyhow::Result` can still use `?` when calling `trace-core` APIs.

## Consequences
Good:
- Public API is consistent and stable.
- `anyhow` remains an internal implementation detail.
- Call sites can still use `anyhow` for rich context.

Bad / costs:
- Extra conversions at crate boundaries when a consumer wants a different error type.

## Alternatives considered
- Expose `anyhow::Result` in public APIs.
  - Why not: couples the public API to `anyhow` and makes future changes harder.
- Define a fully-enumerated error enum.
  - Why not: high maintenance cost for little value in an internal crate; the wrapper achieves the desired surface control.
