# Cleanup Task 046: Tighten C4 and container docs

## Goal

Make the system tour cohesive:
- `docs/architecture/c4.md` stays the top-level narrative (L1 and L2).
- Each container doc is a link-first, low-drift reference with consistent structure and "Related" links.

## Why

The content is generally strong, but there are a few sources of navigation friction and drift risk:
- `docs/architecture/c4.md` and some container docs reference ADRs and other docs as plain text (not clickable links).
- Container docs are inconsistent about having a `## Related` section, which makes it harder to jump to the canonical contracts/specs.
- A few key invariants are duplicated across `c4.md` notes and multiple container docs; that is fine when stable, but it can drift if edited independently.

## Assessment summary (from review task 026)

### C4 purity

`docs/architecture/c4.md` is mostly diagrams plus a short notes section. This is good and should remain the canonical home for C4.

### Missing link hygiene

These should be clickable links, not plain text:
- `docs/architecture/c4.md` - ADR references and `task_lifecycle.md` reference in Notes
- `docs/architecture/containers/rpc_egress_gateway.md` - "See ADR 0002"
- `docs/architecture/containers/delivery_service.md` - "See also" references to `docs/specs/alerting.md` and ADR 0004

### Duplication list (acceptable but should be link-first)

- "Workers have no direct internet egress" appears in:
  - `docs/architecture/c4.md`
  - `docs/architecture/containers/workers.md`
  - `docs/architecture/containers/rpc_egress_gateway.md`
  - `docs/architecture/containers/delivery_service.md`
- "Lambda UDF runner does not connect to Postgres directly" appears in:
  - `docs/architecture/c4.md`
  - `docs/architecture/containers/workers.md`
  - `docs/architecture/contracts/lambda_invocation.md`
  - `docs/architecture/security.md`

## Plan

- In `docs/architecture/c4.md`:
  - Convert ADR references to Markdown links.
  - Convert the `task_lifecycle.md` reference in Notes to a Markdown link.
  - Keep Notes short and pointer-style (avoid restating contracts).
- Standardize container docs with a consistent `## Related` section:
  - Add `## Related` where missing (Dispatcher, Workers, RPC Egress Gateway, Delivery Service).
  - Ensure `## Related` always links to the canonical architecture docs (at minimum: `../c4.md`, `../invariants.md`, `../security.md`, `../operations.md`) plus container-specific contracts/specs/ADRs.
- Ensure all ADR references in container docs are Markdown links (no raw "ADR 000X" text without a link).

## Files to touch

- `docs/architecture/c4.md`
- `docs/architecture/containers/gateway.md` (only if adding standard links)
- `docs/architecture/containers/dispatcher.md`
- `docs/architecture/containers/workers.md`
- `docs/architecture/containers/query_service.md` (only if aligning Related structure; keep it link-first)
- `docs/architecture/containers/rpc_egress_gateway.md`
- `docs/architecture/containers/delivery_service.md`

## Acceptance criteria

- C4 and container docs contain clickable links for ADRs and cross-doc references.
- Every container doc ends with a `## Related` section that points to the canonical owner docs and avoids duplicating deep semantics.
- No new duplication is introduced; any removed text is replaced by links (no information loss).

## Suggested commit message

`docs: tighten c4 and container docs`

