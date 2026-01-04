# Plan

This directory is **sequencing guidance** for implementation. It is intentionally *not normative*; the source of truth for behavior and invariants is:

- `docs/architecture/*` (contracts, lifecycle, containers)
- `docs/specs/*` (feature surfaces)
- `docs/adr/*` (decisions)
- `docs/standards/*` (security + ops invariants)

## How to use this plan

- Keep the **contract-freeze harness** green. If the harness breaks, stop and fix it before adding features.
- Make changes in **small commits** that each come with a clear verification command.
- Treat each milestone below as a review gate: when you hit a **STOP** point, share a repo zip (with `.git`) for review.

### Harness “green” command

From `harness/`:

```bash
docker compose down -v
docker compose up -d
cargo run -- migrate
cargo test -- --nocapture
```

---

## Milestone 0: Contract-freeze harness

Status: **complete** (this is the baseline gate).

Reference:
- `harness/README.md`
- `harness/AGENT_TASKS.md`

STOP: if you change core lifecycle/outbox/token/sink semantics, share a zip for review.

---

## Milestone 1: Lock task capability token contract

Goal: make `/v1/task/*` auth and verification rules match `docs/architecture/contracts.md` exactly, and prove it via harness tests.

### Deliverables
- `/v1/task/*` requires `X-Trace-Task-Capability: <jwt>` on all task-scoped endpoints.
- JWT verification rules are explicit and implemented:
  - required claims (at minimum): `iss`, `aud`, `sub`, `exp`, `iat`, `org_id`, `task_id`, `attempt` (plus optional grants: `datasets`, `s3`)
  - request body `task_id`/`attempt` must match token claims
- Lease fencing remains required for Dispatcher mutations:
  - `(task_id, attempt, lease_token)` must match current row
- Minimal key rotation shape for verifiers (Lite/dev):
  - support `{current_key, next_key}` overlap window (accept either; sign with current)

### Required harness tests
- missing token → 401
- wrong task_id in token → 403/409
- right token but wrong lease_token → 409/403
- token signed with `next_key` accepted during overlap window

### Suggested commits
1. `feat(auth): enforce capability token claims on /v1/task/*`
2. `test(auth): add token mismatch + rotation tests`

✅ Done when: `cd harness && cargo test -- --nocapture` passes.

🛑 STOP: share a zip for review.

---

## Milestone 2: Introduce adapters and AWS implementations (feature-gated)

Goal: keep the orchestration kernel identical across Lite vs AWS; only adapters differ.

### Deliverables
Introduce traits (in a crate/module that is not the harness binary):

- `Queue` (Lite: pgqueue, AWS: SQS)
- `ObjectStore` (Lite: MinIO, AWS: S3)
- `Signer` (Lite: local key, AWS: KMS) — KMS can be stubbed initially if needed

Rules:
- Do **not** change semantics from the harness.
- Keep AWS code behind `--features aws` or a similar compile-time flag.

### Verification
- Harness continues to pass using Lite adapters.
- `cargo check` succeeds with AWS feature enabled (compilation-only is fine at this milestone).

### Suggested commits
1. `refactor(core): introduce Queue/ObjectStore/Signer traits + lite impl`
2. `feat(aws): add sqs + s3 adapters (feature-gated)`

🛑 STOP: share a zip for review.

---

## Milestone 3: Platform-managed Lambda UDF runner (v1 untrusted execution)

Stance:
- **v1 untrusted execution runs on Lambda** (platform-managed runner).
- Untrusted `ecs_udf` is **v2** (see `docs/plan/backlog.md`). Do not implement ECS UDF in v1.

Goal: implement the Lambda runner invocation path and a local version of the runner for tests.

### Deliverables
- One owned struct for the invoke payload used by Dispatcher and runner:
  - `task_id`, `attempt`, `lease_token`, `lease_expires_at`
  - `capability_token`
  - `bundle_url` (pre-signed GET; `bundle_url` remains an accepted alias)
  - `work_payload` (opaque JSON)
- Local runner implementation for harness:
  - fetch bundle via `bundle_url`
  - run Node and Python bundles (TypeScript compiles to JS)
  - runner calls:
    - `/v1/task/buffer-publish`
    - `/v1/task/complete`
- Runner does **not** receive platform secrets (no Secrets Manager, no DB creds).

### Verification
- Add a harness test that exercises:
  - claim → invoke runner → buffer-publish → complete → sink insert (dedupe)
- Manual run remains possible:
  - dispatcher + sink + worker/runner + enqueue

### Suggested commits
1. `feat(udf): define runner payload contract`
2. `feat(udf): implement node+python local runner`
3. `test(udf): add end-to-end runner test`

🛑 STOP: share a zip for review.

---

## Milestone 4: Query Service is explicitly gated (do not build early)

Do not implement Query Service until you can enforce (and test) the sandbox constraints in `docs/architecture/containers/query_service.md`.

Minimum gate:
- negative tests proving SQL cannot:
  - load/execute extensions
  - read local files
  - read URLs (HTTP/HTTPS) or other external resources
  - attach arbitrary databases/files

If you can’t enforce these, do **not** ship `/v1/query` yet.

🛑 STOP: share a zip before starting Query Service implementation.

---

## Review packaging: what to send at STOP points

At each STOP point, share:
- a zip of the repo including `.git`
- output of `cd harness && cargo test -- --nocapture`
- `git log --oneline -n 30`
- a short note: “what changed” + “what you want reviewed”
