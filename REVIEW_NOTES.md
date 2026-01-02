# Architecture Review Notes

Independent reviews of gaps and brittleness in the Trace architecture docs.

---

## Review: Claude Opus 4.5 (2026-01-01)

### Brittle Areas (Control Path)

| Area | Issue | Risk |
|------|-------|------|
| **Partition reconciliation** | "Dispatcher must periodically reconcile" — no spec for interval, detection method, or alerting when partitions go stale | Silent data staleness |
| **Source restart** | "Dispatcher ensures running" — no heartbeat SLA, no restart delay spec, no alerting threshold | Unnoticed source outages |
| **Lease duration** | Never specified anywhere; too short = spurious timeouts, too long = stuck tasks block DAG | Tuning guesswork |
| **Outbox retry** | Has `attempts` counter but no max_attempts, no backoff formula, no DLQ equivalent | Poison rows spin or die silently |
| **Backfill trigger** | Mentioned as a concept, no API contract or endpoint defined | Manual SQL intervention required |
| **GC / retention** | "v1: manual purge" — no tooling, no alerts for storage growth | S3 cost creep |
| **Rollback in-flight cancel** | Cooperative via `status: "Canceled"` — relies on worker polling; worst case waits for lease expiry | Slow rollback |

### Additional Gaps (Not in Brittle List)

| Gap | Location | Notes |
|-----|----------|-------|
| **Scaling enforcement** | `jobs.scaling.max_concurrency` declared but no doc on how Dispatcher enforces it | Is it advisory or hard cap? |
| **Heartbeat interval** | Workers "periodically heartbeat" — no recommended interval vs lease duration | Should be lease_duration / 3 or similar |
| **Buffer queue DLQ handling** | ADR 0006 mentions DLQ but no operational runbook for replay | What's the manual recovery path? |
| **Query Service timeout/limits** | No spec for max query duration, max result size before export-to-S3 kicks in | Could OOM or timeout silently |
| **Delivery Service retry policy** | "At-least-once" with backoff mentioned but no max_attempts or circuit breaker | Could retry forever on permanent failures |
| **Capability token TTL** | "Short-lived JWT" — how short? What happens if task runs longer? | Token expiry mid-task? |
| **Event deduplication window** | Events are at-least-once; is there a dedup window or is it unbounded? | Memory/storage for dedup state |

### Structural/Doc Issues

| Issue | Notes |
|-------|-------|
| **Spec template missing operational detail section** | Template is optimized for changes to existing systems, not greenfield; needs behavior matrices, config examples |
| **ADRs in two places** | `docs/adr/` (empty except template) vs `docs/architecture/adr/` (actual ADRs) |
| **features/ vs specs/** | `specs/` has only templates; `features/` has the actual designs — unclear lifecycle |
| **Glossary in two places** | `docs/readme.md` and `docs/standards/docs_hygiene.md` both define terms |

---

## Review: GPT-5 Codex (2026-01-02)

### Additional Gaps (not covered above)
- **Auth posture drift** — Recent edits shift from “mTLS required + Lambda trusted-only” to capability tokens/worker tokens and “Lambda may be untrusted.” Other docs still say Lambda is trusted-only. Needs a single canonical stance to avoid conflicting guidance.
- **Lifecycle/retention specifics** — No TTL/cleanup policy for staging prefixes, scratch/query exports, or delivery ledgers (`alert_deliveries`); the general “manual purge” note doesn’t cover these, so cost bloat/PII retention risk remains.
- **Backfill/repair semantics** — Beyond deploy/rematerialize, there’s no contract for manual backfill/repair (how tasks are created, how they interact with invalidations/dataset_versions, and how retries/leases behave), leaving operators to improvise.
- **Canonical doc mapping** — No explicit “source of truth” pointer for orchestration/auth/data lifecycle across `readme.md`, `security_model.md`, `contracts.md`, and ADRs, so the ongoing rewrites risk further drift.
