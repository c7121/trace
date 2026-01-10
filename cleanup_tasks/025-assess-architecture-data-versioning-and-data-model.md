# Review Task 025: Data versioning and data model docs

## Scope

- `docs/architecture/data_versioning.md`
- `docs/architecture/data_model/`
- `docs/architecture/db_boundaries.md`

## Goal

Critically assess whether data behavior, schema mapping, and DB boundaries are coherent and not scattered.

## Assessment checklist

- Ownership: does behavior live in architecture, while schema mapping lives in data_model, with no duplicates?
- Canonical source: do docs consistently defer to migrations as canonical DDL?
- Drift: are column lists and SQL snippets repeated across multiple docs?
- Navigation: can a reader find the schema and the behavior from either side in 1-2 clicks?
- Terminology: do we use consistent language for ranges, versions, cursors, invalidations?

## Output

- A critique of the current split and whether the folder structure supports it.
- A proposed "schema mapping" convention across data_model docs.
- A list of redundant sections to move or replace with links (no info loss).

