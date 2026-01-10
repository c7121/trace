# Data model

This directory documents Trace's data model at an architectural level: entity responsibilities, relationships, and key invariants.

Canonical schema source of truth:
- Postgres state schema: `harness/migrations/state/` (applied in order)
- Postgres data schema: `harness/migrations/data/` (applied in order)

Docs in `docs/architecture/data_model/` are not the canonical schema. Column lists in [`state_schema.md`](state_schema.md) and [`data_schema.md`](data_schema.md) are human-readable sketches and may drift.

Start here:
- ERD overview: [erd.md](erd.md)
- State ERD: [erd_state.md](erd_state.md)
- Data ERD: [erd_data.md](erd_data.md)
- State schema: [state_schema.md](state_schema.md)
- Data schema: [data_schema.md](data_schema.md)
- Data versioning schema mapping: [data_versioning.md](data_versioning.md)
- Orchestration semantics: [orchestration.md](orchestration.md)
