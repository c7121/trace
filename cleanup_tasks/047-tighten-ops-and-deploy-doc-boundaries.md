# Cleanup Task 047: Tighten operations and deployment doc boundaries

## Goal

Make operations guidance and deployment guidance coherent and non-duplicative:
- `docs/architecture/operations.md` owns operational defaults, runbooks, and alert thresholds.
- `docs/deploy/*` owns environment topology and deployment steps.
- Deployment docs do not restate architecture contracts; they link to the canonical architecture docs.

## Why

The docs mostly have the right split, but there are a few drift and navigation hazards:
- `docs/architecture/operations.md` claims to be self-contained, which encourages duplication with `invariants.md`, `task_lifecycle.md`, and `security.md`.
- `docs/deploy/infrastructure.md` describes a `/terraform` layout that does not exist in this repo, which is misleading and not actionable.
- Some cross-doc references in deployment docs are plain file paths or backticks, not Markdown links.
- Rollback is described in multiple places (infra rollback vs data cutover rollback) without an explicit boundary.

## Assessment summary (from review task 027)

### What is working

- `docs/deploy/monitoring.md` is intentionally small and correctly defers numeric thresholds to `docs/architecture/operations.md`.
- `docs/deploy/deployment_profiles.md` is link-first and keeps Lite details anchored.
- `docs/architecture/dag_deployment.md` cleanly owns deploy and cutover semantics without restating YAML field definitions.

### Key issues to address

- **Actionability drift:** `docs/deploy/infrastructure.md` references a Terraform directory that is not present in this repo.
- **Boundary ambiguity:** `docs/deploy/infrastructure.md` includes a large "Key Resources" section that mixes:
  - infra topology, and
  - architecture invariants and component responsibilities.
- **Rollback duplication:** `docs/deploy/infrastructure.md` has a generic rollback section; `docs/architecture/dag_deployment.md` defines atomic cutover rollback. The two should be explicitly linked and scoped.
- **Link hygiene:** deployment docs should use clickable Markdown links for references to architecture docs and ADRs.

## Plan

- In `docs/architecture/operations.md`:
  - Add a short doc-ownership header: this doc owns defaults, thresholds, and ops runbooks.
  - Replace the "self-contained" framing with link-first navigation to the canonical owners:
    - `docs/architecture/invariants.md`
    - `docs/architecture/task_lifecycle.md`
    - `docs/architecture/security.md`
  - Keep the numeric defaults table centralized here (as intended), but ensure text sections do not restate deep semantics already owned elsewhere.
- In `docs/deploy/infrastructure.md`:
  - Either remove the nonexistent Terraform tree, or mark it explicitly as a planned target structure and note that it is not implemented in this repo.
  - Restructure "Key Resources" to be link-first:
    - keep infra-specific constraints and AWS-only details,
    - link to container docs and architecture contracts for semantics.
  - Make doc references Markdown links (not backticks).
  - Update "Rollback" to explicitly split:
    - infra rollback (ECS/Terraform), and
    - orchestration rollback (atomic cutover), linking to `docs/architecture/dag_deployment.md` and ADR 0009.
- In `docs/deploy/README.md`:
  - Add an explicit link to `docs/architecture/operations.md` as the canonical home for defaults and operational runbooks.

## Files to touch

- `docs/architecture/operations.md`
- `docs/architecture/dag_deployment.md` (only for a Related link if helpful)
- `docs/deploy/README.md`
- `docs/deploy/infrastructure.md`

## Acceptance criteria

- Deployment docs are actionable and do not imply in-repo infra code that does not exist.
- Operations doc keeps numeric thresholds centralized but becomes link-first for deep semantics.
- Rollback is described with a clear boundary between infra rollback and atomic cutover rollback.
- All cross-doc references are clickable Markdown links.

## Suggested commit message

`docs: tighten ops and deploy boundaries`

