# Docs cleanup tasks

This folder contains selectable, bite-sized documentation cleanup tasks. Each task is a single Markdown file.

How to use:
- Pick one task file.
- Tell me which task to apply.
- I will implement only that task, keeping the diff focused.

Task list (recommended order):
- `cleanup_tasks/015-demote-canonical-ddl-and-dedrift-data-model-docs.md`: Remove drift and contradictions in `docs/architecture/data_model/` and clarify migrations as source of truth.
- `cleanup_tasks/016-move-operator-recipes-to-examples.md`: Move operator "Recipe" sections into `docs/examples/` to reduce operator doc section sprawl.
- `cleanup_tasks/017-fix-mermaid-label-parentheses.md`: Fix Mermaid label text that contains parentheses across docs.
- `cleanup_tasks/020-clarify-data-versioning-doc-ownership.md`: Clarify behavior vs schema ownership for data versioning docs.

Completed:
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
- `014-normalize-block-range-semantics`: normalized block range semantics across contracts, operator specs, and planning docs (start-inclusive, end-exclusive).
- `018-modularize-interface-contracts`: split `docs/architecture/contracts.md` into smaller focused contract docs under `docs/architecture/contracts/` and made `docs/architecture/contracts.md` an index.
- `019-rehome-operator-docs-as-specs`: moved operator docs under `docs/specs/operators/`, added status lines, and updated links.
