# Review Task 033: Alerting spec

## Scope

- `docs/specs/alerting.md`

## Goal

Critically assess whether alerting is described as a coherent surface without duplicating operator and data model details.

## Assessment checklist

- Ownership: what does this spec own vs what belongs in operators and data_model docs?
- Duplication: where are tables, payloads, or flows repeated?
- Correctness: are idempotency and dedupe assumptions explicit and consistent?
- Navigation: can a reader find the operator(s) and schema from this spec quickly?

## Output

- A critique and a recommended doc boundary for alerting.
- A list of link-first changes to reduce duplication.
- Any missing doc links that would reduce scatter.

