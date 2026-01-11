# Docs cleanup tasks

This folder contains selectable, bite-sized documentation cleanup tasks. Each task is a single Markdown file.

How to use:
- Pick one task file.
- Tell me which task to apply.
- I will implement only that task, keeping the diff focused.

Task list (recommended order):
## Implementation tasks (recommended order)

- `cleanup_tasks/046-tighten-c4-and-container-docs.md`: Keep C4 as the top narrative and make container docs consistently link-first with standardized Related links.
- `cleanup_tasks/047-tighten-ops-and-deploy-doc-boundaries.md`: Make operations and deployment guidance link-first, non-duplicative, and actionable (no drift).
- `cleanup_tasks/048-tighten-specs-index-and-templates.md`: Make the specs entrypoint and templates encode governance rules and doc constraints.
- `cleanup_tasks/049-tighten-dag-configuration-spec.md`: Make the DAG YAML contract schema-first, self-consistent, and aligned with core concept terminology.
- `cleanup_tasks/050-tighten-chain-sync-and-ingestion-specs.md`: Clarify ownership boundaries across chain sync, ingestion patterns, and the Cryo adapter spike (link-first, no duplication).
- `cleanup_tasks/051-tighten-query-service-specs.md`: Make Query Service query specs consistent and link-first, including Lite token semantics vs AWS OIDC.
- `cleanup_tasks/052-tighten-udf-specs.md`: Make UDF specs coherent and link-first, and resolve bundle manifest contract drift.
- `cleanup_tasks/053-tighten-alerting-spec.md`: Make the alerting spec link-first and consistent with alert operator examples and ADR decisions.
- `cleanup_tasks/054-tighten-metadata-spec.md`: Make metadata and lineage docs link-first and correct about schema ownership.
- `cleanup_tasks/055-rehome-trace-core-error-contract.md`: Move trace-core error contract to an ADR and remove it from feature specs.
- `cleanup_tasks/056-tighten-operator-specs-catalog.md`: Standardize operator doc structure and make the operator catalog the clear entrypoint.
- `cleanup_tasks/057-tighten-adrs-links-and-focus.md`: Make ADRs link-first and decision-focused (add related links and index summaries).
- `cleanup_tasks/058-tighten-examples-diagnostics-and-runbooks.md`: Make diagnostics docs correct and link-first, and reduce duplication with runnable harness diagnostics.
- `cleanup_tasks/059-tighten-trace-lite-example-guide.md`: Make the Trace Lite end-to-end guide more link-first and less repetitive without losing key details.
- `cleanup_tasks/060-tighten-planning-docs-entrypoints.md`: Make planning docs link-first and clarify what belongs in plan docs vs specs and architecture.
- `cleanup_tasks/061-shrink-milestones-ledger.md`: Reduce `docs/plan/milestones.md` section count by moving completed milestone notes into an archive without losing history.
- `cleanup_tasks/062-tighten-harness-docs-entrypoint.md`: Make harness docs link-first, harness-scoped, and consistent with current plan and contract locations.
- `cleanup_tasks/063-fix-trace-lite-entrypoint-links.md`: Route repo entrypoints to the canonical Trace Lite runbook, keeping harness docs as a secondary reference.
- `cleanup_tasks/064-tighten-agent-docs-entrypoint.md`: Make `docs/agent/` docs discoverable from `AGENTS.md` without expanding the main docs portal.
- `cleanup_tasks/065-reconcile-agent-doc-sync-with-doc-style-rules.md`: Resolve the mismatch between synced `docs/agent/*` content and repo doc style rules.

## Critical assessment tasks (recommended order)

All critical assessment tasks are complete.

Completed:
- `045-tighten-data-versioning-and-data-model-docs`: implemented; made schema mapping link-first, added missing audit tables to the data schema sketch, removed duplicated DDL blocks from domain docs, and aligned range examples to end-exclusive semantics.
- `044-tighten-security-and-contract-doc-ownership`: implemented; clarified invariants vs security model ownership, refined task principal categories, and made v1 org derivation explicit in user API contracts.
- `043-tighten-correctness-docs`: implemented; made correctness docs link-first, clarified runtime lambda vs ecs in task lifecycle, and aligned event flow wording with outbox durability.
- `042-tighten-architecture-index-core-concepts`: implemented; added Dataset and Operator core concepts with best-next-hop links and added `db_boundaries.md` to the recommended reading order.
- `041-tighten-docs-navigation-entrypoints`: implemented; entrypoints link to canonical README pages, `docs/architecture/containers/README.md` added, and the unlinked `docs/architecture/operators/README.md` stub removed.
- `040-audit-orphaned-or-duplicate-docs`: review complete; follow-up tasks `064-tighten-agent-docs-entrypoint` and `065-reconcile-agent-doc-sync-with-doc-style-rules` created.
- `039-assess-harness-docs-entrypoint`: review complete; follow-up tasks `062-tighten-harness-docs-entrypoint` and `063-fix-trace-lite-entrypoint-links` created.
- `021-assess-docs-portal-and-entrypoints`: review complete; follow-up task `041-tighten-docs-navigation-entrypoints` created.
- `022-assess-architecture-index-and-core-concepts`: review complete; follow-up task `042-tighten-architecture-index-core-concepts` created.
- `023-assess-architecture-correctness-and-lifecycle`: review complete; follow-up task `043-tighten-correctness-docs` created.
- `024-assess-architecture-security-and-contracts`: review complete; follow-up task `044-tighten-security-and-contract-doc-ownership` created.
- `025-assess-architecture-data-versioning-and-data-model`: review complete; follow-up task `045-tighten-data-versioning-and-data-model-docs` created.
- `026-assess-architecture-c4-and-containers`: review complete; follow-up task `046-tighten-c4-and-container-docs` created.
- `027-assess-architecture-operations-and-deployment`: review complete; follow-up task `047-tighten-ops-and-deploy-doc-boundaries` created.
- `028-assess-specs-index-and-governance`: review complete; follow-up task `048-tighten-specs-index-and-templates` created.
- `029-assess-spec-platform-surface-dag-config`: review complete; follow-up task `049-tighten-dag-configuration-spec` created.
- `030-assess-specs-chain-sync-and-ingestion`: review complete; follow-up task `050-tighten-chain-sync-and-ingestion-specs` created.
- `031-assess-specs-query-surface`: review complete; follow-up task `051-tighten-query-service-specs` created.
- `032-assess-specs-udf-surface`: review complete; follow-up task `052-tighten-udf-specs` created.
- `033-assess-specs-alerting-surface`: review complete; follow-up task `053-tighten-alerting-spec` created.
- `034-assess-specs-metadata-and-error-contracts`: review complete; follow-up tasks `054-tighten-metadata-spec` and `055-rehome-trace-core-error-contract` created.
- `035-assess-operator-specs-catalog`: review complete; follow-up task `056-tighten-operator-specs-catalog` created.
- `036-assess-adrs-structure-and-durability`: review complete; follow-up task `057-tighten-adrs-links-and-focus` created.
- `037-assess-examples-folder-cohesion`: review complete; follow-up tasks `058-tighten-examples-diagnostics-and-runbooks` and `059-tighten-trace-lite-example-guide` created.
- `038-assess-planning-docs-cohesion`: review complete; follow-up tasks `060-tighten-planning-docs-entrypoints` and `061-shrink-milestones-ledger` created.
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
