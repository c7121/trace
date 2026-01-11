# Cleanup Task 056: Tighten operator specs catalog and doc structure

## Goal

Make the operator docs easy to scan and low-drift by:
- standardizing structure across `docs/specs/operators/*.md`,
- making the operator catalog (`docs/specs/operators/README.md`) the clear entrypoint, and
- keeping narrative workflows in `docs/examples/` without duplicating recipes in operator docs.

## Why

The operator docs are already mostly reference-like and the recipes are short, but there are a few drift risks:

- Status formatting is inconsistent (`Status: Planned` in operator docs vs `planned` in the catalog table).
- Operator docs vary in headings and level of detail, which increases lookup time and makes it harder to see what is contract vs example.
- Recipe links are duplicated across `docs/specs/operators/README.md` and `docs/examples/README.md`. This is not harmful, but it is easy to let the lists drift.

## Plan

- Standardize operator doc structure:
  - Keep a consistent top matter across operator docs:
    - `Status:` with a consistent vocabulary (`planned`, `implemented (Lite)`, `implemented (AWS)`, or `implemented (Lite,AWS)`).
    - `Overview` table with a consistent set of fields (omit only when not applicable):
      - Runtime, Activation, Execution Strategy, Idle Timeout, Source Kind, Image.
  - Normalize common headings (when applicable):
    - Inputs
    - Outputs
    - Configuration
    - Behavior
    - Reliability and idempotency
    - Example DAG config
    - Related
  - Keep operator docs reference-first:
    - Do not include long "Problem/Solution" narratives.
    - Link to a recipe in `docs/examples/` when one exists.

- Tighten the operator catalog:
  - Ensure the catalog `Status` column matches each operator doc `Status:` line.
  - Replace the duplicated recipe list with a link to `docs/examples/README.md` (or make it explicit that the operator catalog list is "selected recipes" and keep it in sync).
  - Add a short legend that explains the status values and what "planned" means for v1 (surface described, but not implemented).

- Validate example snippets for drift:
  - Ensure Example DAG config blocks match the current DAG YAML schema in `docs/specs/dag_configuration.md`.
  - Avoid showing reserved or undefined fields in operator examples.

## Files to touch

- `docs/specs/operators/README.md`
- `docs/specs/operators/*.md` (as needed for structure and status normalization)
- `docs/examples/README.md` (only if deduplicating the recipe list requires it)

## Acceptance criteria

- Operator docs share a consistent, reference-first structure.
- The operator catalog is the clear entrypoint and its statuses match the underlying operator docs.
- Recipes live in `docs/examples/` and operator docs link to them without duplicating long narratives.

## Suggested commit message

`docs: tighten operator specs catalog`
