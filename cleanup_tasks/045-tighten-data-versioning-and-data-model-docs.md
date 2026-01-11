# Cleanup Task 045: Tighten data versioning and data model docs

## Goal

Make the data versioning behavior doc and the data model docs coherent, link-first, and low-drift, with one obvious place to look for:
- behavior contracts (what the system does), and
- schema mapping (where the behavior is stored).

## Why

Right now the top-level split is good:
- `docs/architecture/data_versioning.md` defines behavior and invariants.
- `docs/architecture/data_model/` is intended to be DDL-level schema mapping.

But inside `docs/architecture/data_model/` there is avoidable duplication and drift risk because the same tables are defined in multiple ways:
- column sketches (`state_schema.md`, `data_schema.md`)
- relationship ERDs (`erd_state.md`, `erd_data.md`)
- per-domain SQL blocks (`orchestration.md`, `query_service.md`, `alerting.md`, `address_labels.md`, `pii.md`)

This makes it harder to tell what is authoritative and increases the chance docs go stale.

## Assessment summary (from review task 025)

### What is working

- `docs/architecture/data_versioning.md` correctly owns incremental processing behavior and links to schema mapping.
- `docs/architecture/data_model/data_versioning.md` is the right kind of mapping doc: short, table-first, key-columns only.
- `docs/architecture/db_boundaries.md` is crisp and makes the "no cross-DB FK" rule unambiguous.
- ERDs are already intentionally relationship-focused, which helps reduce drift.

### Key issues to address

- **Multiple schema representations:** the same table is often specified as both Mermaid columns and SQL, which is redundant and can drift.
- **Internal inconsistency:** `data.query_audit` and `data.user_query_audit` appear in some docs but not others (for example ERD vs schema sketch).
- **Mixed concerns in orchestration doc:** `docs/architecture/data_model/orchestration.md` mixes schema sketch with lifecycle diagrams and orchestration behavior, which belongs in `docs/architecture/task_lifecycle.md` and related architecture docs.
- **Range semantics:** the platform has standardized on start-inclusive, end-exclusive ranges for block partitions; the `row_filter` example for `row_range` invalidations should align with that convention to avoid off-by-one confusion.

## Proposed schema mapping convention

Use one consistent pattern across data model docs:

- **Canonical DDL:** when a table exists in migrations, link to the specific migration file that creates or alters it.
- **Column lists:** live only in `state_schema.md` and `data_schema.md` (Mermaid column sketches).
- **Domain docs (`orchestration.md`, `query_service.md`, `alerting.md`, etc):** do not include full `CREATE TABLE` blocks. Instead:
  - list the relevant tables
  - call out primary keys, uniqueness constraints, and any behavioral invariants
  - link to the schema sketch and migration file where applicable
- **Future tables:** keep them, but mark them explicitly as future and link to the spec that owns the plan.

## Plan

- Update `docs/architecture/data_model/README.md` to document the convention above (short and link-first).
- Update `docs/architecture/data_model/data_schema.md` to include the audit tables (`QUERY_AUDIT`, `USER_QUERY_AUDIT`) so the schema sketch matches the ERD and migrations.
- Update `docs/architecture/data_model/query_service.md` to:
  - cover both audit tables (task and user),
  - replace duplicated SQL with links to `harness/migrations/data/0002_query_audit.sql` and `harness/migrations/data/0003_user_query_audit.sql`.
- Update `docs/architecture/data_model/orchestration.md` to:
  - remove lifecycle diagrams and link to `docs/architecture/task_lifecycle.md`,
  - replace full SQL blocks with a smaller "tables + invariants + links" layout.
- Update `docs/architecture/data_model/alerting.md`, `docs/architecture/data_model/address_labels.md`, and `docs/architecture/data_model/pii.md` to follow the same pattern (no duplicated SQL unless explicitly tied to a migration file).
- Align the `row_filter` example in `docs/architecture/data_model/data_versioning.md` with end-exclusive range semantics.

## Files to touch

- `docs/architecture/data_versioning.md`
- `docs/architecture/db_boundaries.md` (only if needed for link clarity)
- `docs/architecture/data_model/README.md`
- `docs/architecture/data_model/data_versioning.md`
- `docs/architecture/data_model/data_schema.md`
- `docs/architecture/data_model/query_service.md`
- `docs/architecture/data_model/orchestration.md`
- `docs/architecture/data_model/alerting.md`
- `docs/architecture/data_model/address_labels.md`
- `docs/architecture/data_model/pii.md`

## Acceptance criteria

- A reader can find "behavior" vs "schema mapping" in 1-2 clicks from either side.
- Each table has one obvious place where columns are listed (schema sketch) and one place where behavior is defined (architecture/spec).
- Domain docs stop duplicating `CREATE TABLE` blocks and become shorter, more navigable references.
- Range and invalidation examples do not risk an off-by-one interpretation.

## Suggested commit message

`docs: tighten data model schema mapping`

