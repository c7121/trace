# Data model

This directory documents Trace's data model at an architectural level: entity responsibilities, relationships, and key invariants.

## Schema mapping conventions

- **Canonical DDL**: if a table exists in migrations, the migration file is the source of truth:
  - Postgres state: `harness/migrations/state/` (applied in order)
  - Postgres data: `harness/migrations/data/` (applied in order)
- **Column lists**: live only in the schema sketches:
  - Postgres state: [state_schema.md](state_schema.md)
  - Postgres data: [data_schema.md](data_schema.md)
- **ERDs**: relationship-focused views that intentionally omit most columns:
  - Postgres state: [erd_state.md](erd_state.md)
  - Postgres data: [erd_data.md](erd_data.md)
- **Domain docs** (for example `orchestration.md`, `query_service.md`): list tables and invariants and link to schema sketches and migrations. Avoid duplicating full `CREATE TABLE` blocks.
- **Future tables**: are explicitly marked as planned and link to the spec that owns the plan.

If any document here conflicts with migrations for an implemented table, migrations win.

Start here:
- ERD overview: [erd.md](erd.md)
- State ERD: [erd_state.md](erd_state.md)
- Data ERD: [erd_data.md](erd_data.md)
- State schema: [state_schema.md](state_schema.md)
- Data schema: [data_schema.md](data_schema.md)
- Data versioning schema mapping: [data_versioning.md](data_versioning.md)
- Orchestration semantics: [orchestration.md](orchestration.md)
