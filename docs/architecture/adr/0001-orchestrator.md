# ADR 0001: Orchestrator

## Status
- Accepted (revised December 2024)

## Decision
- Build a **custom orchestration system** with:
  - **Dispatcher** — central coordinator, creates tasks, manages state
  - **Trigger Service** — evaluates cron, webhooks, threshold events; emits job requests
  - **Workers** — polyglot containers (Rust, Python, R, Scala, etc.) that execute jobs
  - **SQS** — task queue for push-based dispatch to workers
  - **Postgres** — source of truth for jobs, tasks, triggers, assets, lineage

## Context
- Initially considered Dagster for its asset model and UI.
- Dagster's Python-centric design conflicts with polyglot runtime requirements.
- Need to support Rust, R, Scala, TypeScript workers — not just Python.
- Custom system allows "everything is a job" model with containerized, language-agnostic execution.

## Why
- **Polyglot support** — workers are containers, any runtime.
- **Simpler model** — jobs are the universal primitive; no framework lock-in.
- **Control** — own the scheduler, queue, and state; no black-box limitations.
- **Extensibility** — triggers, execution strategies, and storage are pluggable.

## Consequences
- Must build and maintain dispatcher, trigger evaluation, worker contract.
- Need to implement: task lifecycle, retries, dead-letter, heartbeats, memoization.
- UI/observability is our responsibility (or build on top of standard tooling).
- DAG definitions via YAML, synced to Postgres.

## Trade-offs
- More upfront work vs. Dagster's batteries-included.
- No off-the-shelf UI (can build or use Postgres-backed dashboards).
- Full ownership of failure modes and edge cases.

## Open Questions
- None currently.
