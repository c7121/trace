# Milestones

This file is the canonical ledger for:

- completed milestones (with immutable git tags), and
- planned milestones (next work).

Milestones are defined in more detail (gates, STOP points, scope) in `docs/plan/plan.md`.

## Completed milestones

Each completed milestone is pinned by an annotated git tag `ms/<N>` pointing at the STOP boundary commit.

| Milestone | Tag  | Commit    | Summary |
|----------:|------|-----------|---------|
| 1 | ms/1 | 31c6675 | Harness invariants + token overlap tests |
| 2 | ms/2 | d6b2d2d | Core traits + lite rewiring + aws adapters |
| 3 | ms/3 | 0f8cc26 | Runner payload + invoker/runner E2E + dispatcher client |
| 4 | ms/4 | 9beeb95 | SQL validation gate hardened |
| 5 | ms/5 | 22c3511 | Task query service + audit + deterministic DuckDB fixture |
| 6 | ms/6 | 9578de7 | Dataset grants enforced + docs corrected |
| 7 | ms/7 | 88dbd37 | Harness E2E invariant: dataset grant -> task query -> audit |
| 8 | ms/8 | 2334ae5 | Dispatcher extracted into `crates/trace-dispatcher` (harness wrapper kept) |

### How to review a milestone

Given two milestone tags (example: ms/6 and ms/7):

    git diff --stat ms/6..ms/7
    git log --oneline ms/6..ms/7
    git checkout ms/7

Then run the milestone gates described in `docs/plan/plan.md`.

## Planned milestones (next)

This list is intentionally short; add details in `docs/plan/plan.md`.

| Milestone | Title | Notes |
|----------:|-------|-------|
| 9 | Extract sink into `crates/trace-sink` | Make harness depend on the production sink implementation while keeping invariants green |
| 10 | Introduce `RuntimeInvoker` interface | Local invoker + AWS Lambda invoker; dispatcher uses it to invoke untrusted UDFs |
| 11 | Bundle manifest + bundle store | Signed bundle manifests, size/hash checks, and presigned fetch rules (fail closed) |
| 12 | AWS profile “real wiring” | Use SQS + S3 adapters end-to-end (still minimal; no IaC required yet) |
| 13 | Minimal user API v1 | Bundle upload + DAG registration + alert definition CRUD (no public query surface) |
| 14 | Deployable AWS MVP | IaC, IAM+network boundaries, runbooks, smoke tests |
