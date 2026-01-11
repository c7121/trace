# Docs cleanup tasks

This folder contains selectable, bite-sized documentation cleanup tasks. Each task is a single Markdown file.

How to use:
- Pick one task file.
- Tell me which task to apply.
- I will implement only that task, keeping the diff focused.

Task list (recommended order):
## Implementation tasks (recommended order)

- `cleanup_tasks/041-tighten-docs-navigation-entrypoints.md`: Replace directory links with canonical entrypoints, add a containers index, and resolve the orphan `docs/architecture/operators/README.md`.
- `cleanup_tasks/042-tighten-architecture-index-core-concepts.md`: Improve `docs/architecture/README.md` core concepts and reading order to reduce lookup friction.

## Critical assessment tasks (recommended order)

- `cleanup_tasks/022-assess-architecture-index-and-core-concepts.md`: Assess `docs/architecture/README.md` for ownership, reading order, and discoverability.
- `cleanup_tasks/023-assess-architecture-correctness-and-lifecycle.md`: Assess invariants, task lifecycle, and event flow docs for correctness and duplication.
- `cleanup_tasks/024-assess-architecture-security-and-contracts.md`: Assess security model and contract docs for boundary clarity and drift risk.
- `cleanup_tasks/025-assess-architecture-data-versioning-and-data-model.md`: Assess data versioning behavior and schema docs for structure and duplication.
- `cleanup_tasks/026-assess-architecture-c4-and-containers.md`: Assess C4 and container docs for cohesion and link-first navigation.
- `cleanup_tasks/027-assess-architecture-operations-and-deployment.md`: Assess ops vs deploy docs boundary and eliminate overlap.
- `cleanup_tasks/028-assess-specs-index-and-governance.md`: Assess specs index and templates for structure and workflow fit.
- `cleanup_tasks/029-assess-spec-platform-surface-dag-config.md`: Assess DAG configuration spec clarity and ownership boundaries.
- `cleanup_tasks/030-assess-specs-chain-sync-and-ingestion.md`: Assess chain sync and ingestion specs for cohesion and redundancy.
- `cleanup_tasks/031-assess-specs-query-surface.md`: Assess query specs for surface clarity and duplication.
- `cleanup_tasks/032-assess-specs-udf-surface.md`: Assess UDF specs for boundaries, safety hooks, and duplication.
- `cleanup_tasks/033-assess-specs-alerting-surface.md`: Assess alerting spec for ownership and overlap with operators and data model.
- `cleanup_tasks/034-assess-specs-metadata-and-error-contracts.md`: Assess metadata and error contract specs for completeness and duplication.
- `cleanup_tasks/035-assess-operator-specs-catalog.md`: Assess operator spec catalog vs examples for structure and drift risk.
- `cleanup_tasks/036-assess-adrs-structure-and-durability.md`: Assess ADRs for continued relevance and duplication with specs/architecture.
- `cleanup_tasks/037-assess-examples-folder-cohesion.md`: Assess examples folder for cohesion and discoverability.
- `cleanup_tasks/038-assess-planning-docs-cohesion.md`: Assess planning docs for usefulness, scope, and ownership.
- `cleanup_tasks/039-assess-harness-docs-entrypoint.md`: Assess harness docs as an entrypoint for Trace Lite and verification.
- `cleanup_tasks/040-audit-orphaned-or-duplicate-docs.md`: Audit unlinked or duplicated docs and propose disposition.

Completed:
- `021-assess-docs-portal-and-entrypoints`: review complete; follow-up task `041-tighten-docs-navigation-entrypoints` created.
- `022-assess-architecture-index-and-core-concepts`: review complete; follow-up task `042-tighten-architecture-index-core-concepts` created.
- `001-slim-docs-portal`: `docs/README.md` is now a portal; product overview moved to `README.md`; design principles moved to `docs/architecture/invariants.md`.
- `002-standardize-docs-entrypoint`: renamed the docs entrypoint to `docs/README.md` and updated references.
- `003-remove-standards-folder`: rehomed security and operations under `docs/architecture/`; folded doc ownership into `docs/architecture/README.md`; removed `docs/standards/`.
- `004-consolidate-query-service-docs`: trimmed `docs/architecture/containers/query_service.md` to be link-first; moved non-C4 details into specs and ops/monitoring docs.
- `005-consolidate-dispatcher-docs`: trimmed `docs/architecture/containers/dispatcher.md` to be link-first; moved credential minting contract to `docs/architecture/contracts.md`; linked lifecycle to `docs/architecture/task_lifecycle.md`.
- `006-merge-dag-configuration-docs`: made `docs/specs/dag_configuration.md` config-only and `docs/architecture/dag_deployment.md` deploy-only; removed overlap and linkified ownership.
- `007-consolidate-milestone-micro-specs`: folded milestone micro-spec content into `docs/plan/milestones.md`; removed obsolete micro specs from `docs/specs/`.
- `008-deploy-docs-reduction`: added `docs/deploy/README.md` entrypoint; moved Trace Lite local sync doc to `docs/examples/`; trimmed `docs/deploy/deployment_profiles.md` to be link-first.
- `010-align-cryo-ingest-operator-doc`: aligned `docs/specs/operators/cryo_ingest.md` to the current `chain_sync` payload contract and the harness worker publication behavior.
- `011-align-cryo-cli-docs`: aligned `harness/NOTES.md` to the Cryo CLI invocation used by the harness cryo worker.
- `012-docs-navigation-workflow`: added a docs workflow section and cross-linked the C4 tour with the AWS infrastructure view.
- `013-specs-index-jtbd`: added `docs/specs/README.md` as an index and linked it from `docs/README.md`.
- `015-demote-canonical-ddl-and-dedrift-data-model-docs`: clarified `harness/migrations/` as the schema source of truth and made ERDs relationship-focused to reduce drift.
- `017-fix-mermaid-label-parentheses`: verified Mermaid label text contains no parentheses; remaining parentheses are only Mermaid shape syntax.
- `020-clarify-data-versioning-doc-ownership`: clarified behavior vs schema ownership and reduced duplication between the behavior contract and the schema mapping.
- `014-normalize-block-range-semantics`: normalized block range semantics across contracts, operator specs, and planning docs (start-inclusive, end-exclusive).
- `018-modularize-interface-contracts`: split `docs/architecture/contracts.md` into smaller focused contract docs under `docs/architecture/contracts/` and made `docs/architecture/contracts.md` an index.
- `019-rehome-operator-docs-as-specs`: moved operator docs under `docs/specs/operators/`, added status lines, and updated links.
- `016-move-operator-recipes-to-examples`: moved operator recipe narratives into `docs/examples/` and replaced in-operator sections with links.
