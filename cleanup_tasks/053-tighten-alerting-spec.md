# Cleanup Task 053: Tighten alerting spec boundaries and contracts

## Goal

Make `docs/specs/alerting.md` a coherent, link-first alerting surface spec that:
- avoids duplicating operator and data model details,
- aligns the alert event record schema with ADR decisions and operator examples, and
- makes it easy to navigate to the right owner doc for delivery, buffered datasets, and UDF trust boundaries.

## Why

Alerting is already mostly link-first, but a few details can mislead implementers:

- `docs/specs/alerting.md` defines a strict alert event record schema and says top-level unknown fields are rejected, but the platform ADRs and operator examples imply additional top-level fields (for example `severity`) are part of the contract.
- The spec mentions Delivery Service but does not link to the Delivery Service container doc or ADR 0004, which owns key alerting design decisions (multi-writer sink, severity taxonomy, delivery idempotency key).
- Some buffered dataset mechanics and refusal behavior are repeated in multiple places and should be owned by `docs/architecture/contracts/buffered_datasets.md` and ADR 0006, with alerting owning only alert-specific row schema and semantics.

## Assessment summary (from review task 033)

### Ownership boundary

- `docs/specs/alerting.md` should own:
  - alerting behavior and surfaces (definitions, evaluation, routing, delivery),
  - the alert event batch artifact record schema and alert-specific invariants (dedupe_key rules, required fields),
  - high-level reorg and invalidation intent (link to data versioning for mechanics).

- Operator-specific config and runtime details should remain owned by:
  - `docs/specs/operators/alert_evaluate.md`
  - `docs/specs/operators/alert_route.md`

- Delivery safety (SSRF defenses, outbound constraints) should remain owned by:
  - `docs/architecture/containers/delivery_service.md`

- Buffered dataset pointer pattern and sink refusal behavior should remain owned by:
  - `docs/architecture/contracts/buffered_datasets.md`
  - ADR 0006

### Drift to address

- ADR 0004 defines a severity taxonomy (`info`, `warning`, `critical`) and the alert sink and delivery model, but `docs/specs/alerting.md` does not link to it and does not list `severity` as an allowed alert event field.
- `docs/specs/operators/alert_route.md` uses `where: severity: critical` in its example, which implies `severity` is a first-class field in `alert_events`.

## Plan

- Tighten `docs/specs/alerting.md`:
  - Add direct Markdown links to:
    - ADR 0004 and ADR 0006
    - `docs/architecture/containers/delivery_service.md`
    - `docs/architecture/contracts/buffered_datasets.md`
    - `docs/specs/operators/alert_evaluate.md` and `docs/specs/operators/alert_route.md`
  - Update the alert event record schema section to explicitly include:
    - `severity` as an allowed field, with the v1 taxonomy from ADR 0004.
    - A clear statement of which fields are producer-supplied vs sink-assigned (at minimum `org_id`, `producer_*` ids are sink-assigned).
  - Keep the strict "unknown fields rejected" rule, but make the allowed field set complete and consistent with operator examples and ADR decisions.
  - Reduce duplication by turning buffered dataset pointer mechanics and sink refusal behavior into a short summary with a link to the canonical contract doc.

## Files to touch

- `docs/specs/alerting.md`

## Acceptance criteria

- A reader can find the alerting pipeline owners in 1-2 clicks (operators, buffered dataset contract, delivery service).
- The alert event record schema is self-consistent and matches ADR 0004 and operator examples (no hidden optional fields).
- The alerting spec is link-first and does not restate buffered dataset mechanics or Delivery Service SSRF details.

## Suggested commit message

`docs: tighten alerting spec`
