# Docs cleanup tasks

This folder contains selectable, bite-sized documentation cleanup tasks. Each task is a single Markdown file.

How to use:
- Pick one task file.
- Tell me which task to apply.
- I will implement only that task, keeping the diff focused.

Task list (recommended order):
- `cleanup_tasks/005-consolidate-dispatcher-docs.md`: Reduce duplication across Dispatcher docs by assigning owners and trimming repeats.
- `cleanup_tasks/006-merge-dag-configuration-docs.md`: Remove overlap between DAG deployment/config docs and make one canonical owner.
- `cleanup_tasks/007-consolidate-milestone-micro-specs.md`: Remove tiny milestone specs by folding into the milestone ledger.
- `cleanup_tasks/008-deploy-docs-reduction.md`: Reduce and reorg deploy docs into a single "how to deploy" path.
- `cleanup_tasks/009-doc-hygiene-sweep.md`: Mechanical hygiene pass (no em dashes, Mermaid label punctuation checks, link validation).

Completed:
- `001-slim-docs-portal`: `docs/README.md` is now a portal; product overview moved to `README.md`; design principles moved to `docs/architecture/invariants.md`.
- `002-standardize-docs-entrypoint`: renamed the docs entrypoint to `docs/README.md` and updated references.
- `003-remove-standards-folder`: rehomed security and operations under `docs/architecture/`; folded doc ownership into `docs/architecture/README.md`; removed `docs/standards/`.
- `004-consolidate-query-service-docs`: trimmed `docs/architecture/containers/query_service.md` to be link-first; moved non-C4 details into specs and ops/monitoring docs.
