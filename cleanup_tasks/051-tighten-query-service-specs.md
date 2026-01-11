# Cleanup Task 051: Tighten Query Service specs

## Goal

Make the Query Service query surface docs consistent, link-first, and low-drift:
- the implemented endpoints and auth contexts are unambiguous,
- Lite dev-only token semantics do not contradict the AWS/OIDC security narrative, and
- cross-cutting hardening and limits are not repeated in multiple specs.

## Why

The current split is close, but there are a few high-risk sources of confusion:

- `docs/specs/query_service_task_query.md` lists `/v1/query` as a non-goal even though it is implemented.
- `docs/specs/query_service_user_query.md` documents a Lite HS256 user JWT that carries dataset and storage grants, while `docs/architecture/security.md` describes a user OIDC JWT as auth-only with authz derived from Postgres state.
- ADR references in query specs are plain text (not links), and some specs repeat runtime hardening guidance that should have one clear owner.

## Assessment summary (from review task 031)

### Ownership and duplication

- The endpoint split is correct:
  - `query_service_task_query.md` owns `POST /v1/task/query`.
  - `query_service_user_query.md` owns `POST /v1/query`.
  - `query_sql_gating.md` owns the allow and deny rules for untrusted SQL.
  - `query_service_query_results.md` owns the future results and export contract.
- Several pieces of cross-cutting guidance are repeated across endpoint specs:
  - limit clamping semantics,
  - DuckDB runtime hardening checklist,
  - operations defaults (timeouts, inline size limits).

### Token model drift risk

In Trace Lite today, `POST /v1/query` uses an HS256 Bearer token whose claims include dataset grants and S3 grants (it is a platform-minted capability-like token, not an IdP OIDC JWT). This needs to be explicit so readers do not apply the AWS/OIDC narrative to Lite behavior.

## Plan

- Tighten `docs/specs/query_service_task_query.md`:
  - Update `Owner` to `Platform`.
  - Remove the "user query endpoint is a non-goal" drift and link to `docs/specs/query_service_user_query.md` instead.
  - Replace repeated DuckDB hardening details with links to the canonical owner doc (see below).
- Tighten `docs/specs/query_service_user_query.md`:
  - Make Lite mode explicit: the Bearer token is a Trace Lite dev token that carries dataset and storage grants.
  - Add a short "AWS future shape" note that points to `docs/architecture/security.md` for the OIDC user principal model, without implying the same token is used in Lite.
  - Linkify ADR references and other doc references (use Markdown links, not plain file paths).
- Make cross-cutting controls link-first:
  - Make `docs/specs/query_sql_gating.md` the single owner for the detailed DuckDB runtime hardening checklist (defense-in-depth), and have both endpoint specs link to it instead of restating settings.
  - Ensure endpoint specs link to `docs/architecture/operations.md` for default timeouts and size caps.
- Keep `docs/specs/query_service_query_results.md` consistent:
  - Ensure exports and batch mode remain link-first for authn/authz and gating.
  - Ensure all ADR and data model references are Markdown links.

Note: any required change to `docs/architecture/security.md` or `docs/architecture/user_api_contracts.md` to clarify Lite vs AWS user JWT semantics should be handled in `cleanup_tasks/044-tighten-security-and-contract-doc-ownership.md`, but the query specs should not remain contradictory in the meantime.

## Files to touch

- `docs/specs/query_service_task_query.md`
- `docs/specs/query_service_user_query.md`
- `docs/specs/query_service_query_results.md`
- `docs/specs/query_sql_gating.md`

## Acceptance criteria

- The implemented endpoints match the specs (no "non-goal" drift).
- It is unambiguous what kind of Bearer token `/v1/query` expects in Trace Lite and how it differs from the AWS/OIDC user principal model.
- Runtime hardening and operations defaults are link-first and not repeated across multiple endpoint specs.
- ADR references and internal doc references are real Markdown links.

## Suggested commit message

`docs: tighten query service specs`
