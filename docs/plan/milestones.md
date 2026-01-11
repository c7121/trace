# Milestones

This file is the canonical ledger for:

- completed milestones (with immutable git tags), and
- planned milestones (next work).

See `AGENTS.md` in the repo root for milestone workflow rules (STOP gates, context links, harness commands).

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
| 9 | ms/9 | 3892487 | Sink extracted into `crates/trace-sink` (harness wrapper kept) |
| 10 | ms/10 | 07a08df | RuntimeInvoker interface (lite + AWS Lambda feature-gated); harness routes UDF invocation via invoker |
| 11 | ms/11 | 319df13 | Parquet dataset versions pinned in task capability tokens; Query Service attaches via trusted manifest |
| 12 | ms/12 | 339bef6 | Cryo ingest worker writes Parquet+manifest to MinIO; registers dataset_versions idempotently |
| 13 | ms/13 | 93e74da | Lite chain sync planner (cursor + scheduled ranges) + harness E2E |
| 14 | ms/14 | 005036d | Alert evaluation over Parquet datasets (QS -> UDF -> sink) + harness E2E |
| 15 | ms/15 | c4cfdaf | Chain sync entrypoint spec locked (no code) |
| 16 | ms/16 | f973a14 | Chain sync job runner (multi-dataset) + runnable examples |
| 17 | ms/17 | 9391915 | `trace-lite` local stack runner + runbook fixes |

### How to review a milestone

Given two milestone tags (example: ms/6 and ms/7):

    git diff --stat ms/6..ms/7
    git log --oneline ms/6..ms/7
    git checkout ms/7

Then run the milestone gates described in `AGENTS.md` (root of repo).

## Planned milestones (next)

Milestones **after ms/17** are sequenced to prove a full **Lite** deployment that can:

- run the platform services locally,
- sync a chain locally using **Cryo**,
- store chain datasets as **Parquet** in object storage (MinIO locally),
- query those datasets safely via Query Service **without** allowing untrusted SQL to read arbitrary files/URLs,
- and only *then* move to AWS deployment.

The table is the short index. Detailed deliverables + STOP gates follow.

| Milestone | Title | Notes |
|----------:|-------|-------|
| 18 | Bundle manifest + multi-language UDF runtime | Signed bundle manifests + hash/size checks; Node/Python and Rust custom runtime |
| 19 | Minimal user API v1 | Bundle upload + DAG registration + publish datasets + alert definition CRUD |
| 20 | AWS deployable MVP | IaC + IAM/network boundaries + S3/SQS/Lambda wiring + smoke tests |
| S1 | Security gate: Query Service egress allowlist | Mandatory before any non-dev deployment that allows remote Parquet scans |

---

Note: detailed completed milestone notes live in `milestones_archive.md`.
Milestone notes are historical; current behavior is defined by `docs/architecture/*` and `docs/specs/*`.

## Milestone 18: Bundle manifest + multi-language UDF runtime

Goal: replace harness-only runner logic with a real bundle model that is safe under retries and supports multiple languages.

Notes:
- Signed manifests, hash/size checks, and fail-closed fetch/execution rules.
- Node/Python and Rust custom runtime support using common packaging/tooling.

### Context links
- `docs/specs/udf_bundle_manifest.md`
- `docs/specs/udf.md`
- `docs/adr/0003-udf-bundles.md`

### Deliverables
- Implement `bundle_manifest.json` validation and fail-closed extraction rules in `trace-core`.
- Execute Node and Python bundles using the manifest entrypoint contract.
- Add Rust custom runtime support using the AWS Lambda `bootstrap` convention:
  - accept `runtime: rust` in the bundle manifest
  - execute the `bootstrap` entrypoint with stdin invocation JSON and stdout JSON result (same contract as Node/Python)
- Add harness tests covering Node, Python, and Rust bundles.

### STOP gate
- `cd harness && cargo test -- --nocapture`

---

## Milestone 19: Minimal user API v1

Goal: expose only the smallest stable public surface (everything else remains internal).

Note: `POST /v1/query` is already implemented in `trace-query-service` (commit: `ce206d7`).

Deliverables:
- Bundle upload + DAG registration
- Publish datasets (make chain sync datasets queryable by users)
- `POST /v1/query` - user-facing interactive query endpoint (Query Service)
  - Implemented minimal in Lite: Bearer JWT with dataset grants, inline JSON results only
  - Future: dataset registry lookup, OIDC/JWKS verification, result persistence and exports
- Alert definition CRUD

---

## Milestone 20: AWS deployable MVP

Goal: move the proven Lite semantics to AWS adapters + deployable infra (S3/SQS/Lambda/IAM/VPC).
---

## Security Gate S1: Query Service egress allowlist

### Why this exists
If Query Service is allowed to scan *authorized* remote Parquet datasets (HTTP/S3) during query execution, DuckDB becomes a **network-capable** process.

If the SQL gate is ever bypassed (bug, misconfiguration, future feature), an attacker could try to use Query Service as an SSRF / exfiltration primitive.

This gate makes the trust boundary enforceable by requiring **OS/container-level egress allowlists**.

### Scope
This gate applies to **Query Service** only.

(It does **not** replace the existing “no third-party internet egress” requirement for untrusted UDF runtimes; that requirement remains and is tracked elsewhere.)

### Context links
- `docs/specs/query_sql_gating.md` (notes on remote Parquet + sandbox)
- `docs/specs/query_service_task_query.md` (task query threat model)
- `docs/architecture/containers/query_service.md` (DuckDB hardening + attach strategy)
- `docs/adr/0002-networking.md` (no egress by default + egress services)
- `docs/architecture/security.md` (egress allowlists)

### Deliverables
- **Lite/local** (ms/15): document the local posture explicitly:
  - By default, Lite/dev may not enforce strict egress controls.
  - If remote Parquet scans are enabled, provide a recommended enforcement approach (container network policy / host firewall) and a verification checklist.
- **AWS** (ms/18): enforce a strict egress allowlist:
  - Query Service must run in private subnets with **no general NAT egress**.
  - Allow egress only to:
    - the configured object store endpoint(s) (S3 via VPC endpoint), and
    - internal platform services as required.
  - “Only Delivery Service and RPC Egress Gateway have outbound internet egress” remains true.

### Verification
- From inside the Query Service container/task:
  - Object store endpoint is reachable.
  - An arbitrary public endpoint is **not** reachable (fail closed).
- Keep the SQL gate tests green (this gate is defense-in-depth, not a replacement).
