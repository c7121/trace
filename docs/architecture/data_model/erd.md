# Entity Relationship Diagram

This repo uses **two** Postgres instances:

- **Postgres state**: orchestration + lineage + dataset registry (system of record)
- **Postgres data**: user-facing datasets + query/alert tables

There are **no cross-DB foreign keys**. Any `org_id`/`user_id`/`task_id` columns in Postgres data are **soft references**
to Postgres state and must be validated at service boundaries.

Schema source of truth and scope: [README.md](README.md).

- State ERD: [erd_state.md](erd_state.md)
- Data ERD: [erd_data.md](erd_data.md)
