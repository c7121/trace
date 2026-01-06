# ADR 0001: Orchestrator

## Status
- Accepted (revised December 2024)

## Decision
- Build a **custom orchestration system** with:
  - **Dispatcher** - central coordinator, creates tasks, routes upstream events, manages state
  - **Lambda Sources** - cron/webhook/manual sources implemented as Lambda runtime
  - **Workers** - polyglot containers (Rust, Python, etc.) that execute jobs
  - **SQS** - task queue for push-based dispatch to workers
  - **Postgres state** - source of truth for jobs, tasks, assets, lineage

## Context
- Initially considered Dagster for its asset model and UI.
- Rejected Dagster because:
  - Code changes trigger downstream reruns (not desirable for our use case)
  - Single runtime - Python-centric, conflicts with polyglot requirements
  - DAGs defined in code rather than config (we want YAML-based, version-controlled config)
- Need to support Rust and TypeScript workers - not just Python.
- Custom system allows "everything is a job" model with containerized, language-agnostic execution.

## Why
- **Polyglot support** - workers are containers, any runtime.
- **Simpler model** - jobs are the universal primitive; no framework lock-in.
- **Control** - own the scheduler, queue, and state; no black-box limitations.
- **Extensibility** - activation modes, execution strategies, and storage are pluggable.

## Consequences
- Must build and maintain dispatcher, event routing, worker contract.
- Need to implement: task lifecycle, retries, dead-letter, heartbeats, memoization.
- UI/observability is our responsibility (or build on top of standard tooling).
- DAG definitions via YAML, synced to Postgres state.

## Trade-offs
- More upfront work vs. Dagster's batteries-included.
- No off-the-shelf UI (can build or use Postgres state-backed dashboards).
- Full ownership of failure modes and edge cases.

