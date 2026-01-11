# Cleanup Task 043: Tighten correctness docs

## Goal

Make the correctness narrative cohesive and non-duplicative across:
- `docs/architecture/invariants.md`
- `docs/architecture/task_lifecycle.md`
- `docs/architecture/event_flow.md`

## Why

These docs describe the same system truths (at-least-once, fencing, leases, outbox, wake-up queues) in multiple places. That increases drift risk and forces readers to hunt for what is canonical.

## Plan

- Make doc ownership explicit:
  - `invariants.md` is the canonical list of enforceable truths.
  - `task_lifecycle.md` explains how those truths are achieved operationally.
  - `event_flow.md` is a top-level sequence view that links out for details.
- Reduce duplication by replacing repeated narrative with links:
  - Keep short summaries where needed, but make them defer to `invariants.md`.
- Clarify Lambda vs ECS execution:
  - Ensure `task_lifecycle.md` explains the `runtime: lambda` path (no queue wake-up or claim call, but still attempt-fenced and lease-gated mutations).
- Align event flow terminology with durability mechanisms:
  - Replace any ambiguous "persist event" language with outbox or state writes where appropriate.
- Link hygiene:
  - Replace raw path mentions with Markdown links where it improves navigation.

## Files to touch

- `docs/architecture/invariants.md`
- `docs/architecture/task_lifecycle.md`
- `docs/architecture/event_flow.md`

## Acceptance criteria

- A reader can answer "what is the invariant" vs "how it works" without reading three versions of the same text.
- The docs do not contradict each other or the contract docs.
- No information loss: content is moved, condensed, or replaced with links to the canonical owner.

## Suggested commit message

`docs: tighten correctness docs`

