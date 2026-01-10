# Review Task 031: Query specs and surfaces

## Scope

- `docs/specs/query_service_user_query.md`
- `docs/specs/query_service_task_query.md`
- `docs/specs/query_service_query_results.md`
- `docs/specs/query_sql_gating.md`

## Goal

Critically assess whether the query surface is well-defined, non-duplicative, and safe-by-default.

## Assessment checklist

- Ownership: does each doc own a single surface (user vs task vs results vs gating)?
- Duplication: are the same semantics defined more than once?
- Security: are the key safety guarantees and non-goals visible and consistent?
- Linkability: do specs link out to architecture invariants and contracts rather than restating them?

## Output

- A critique of the current split, including any missing or redundant docs.
- A list of sections to link or move to reduce repetition.
- A suggested "start here" reading path for query changes.

