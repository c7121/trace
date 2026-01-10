# Cleanup Task 003: Remove `docs/standards/`

## Stance
The current `docs/standards/` folder is a grab bag. Keeping it increases doc surface area and makes "source of truth" ambiguous.

## Goal
Rehome the useful, enforceable material into the existing architecture docs, fold checklists into the right place, and remove `docs/standards/` entirely.

## Recommendation
- Treat "trust boundaries and enforceable invariants" as architecture.
- Treat "checklists and authoring rules" as contributor process (agent or repo meta), not platform behavior.

## Plan
1. Rehome and reduce:
   - Move `docs/standards/security_model.md` to `docs/architecture/security.md` and trim any repeated content that is already covered by:
     - `docs/architecture/contracts.md`
     - `docs/architecture/user_api_contracts.md`
     - container docs where appropriate
   - Move `docs/standards/operations.md` to `docs/architecture/operations.md` and trim repeated content already covered by `docs/architecture/invariants.md` and `docs/architecture/task_lifecycle.md`.
2. Fold or relocate checklists:
   - Move `docs/standards/docs_hygiene.md` to `docs/agent/docs_hygiene.md` (or fold key rules into `docs/architecture/README.md` if you want fewer files).
   - Decide on `docs/standards/security_hardening.md`:
     - Preferred: merge into `docs/architecture/security.md` as a short "Implementation checklist" section and delete the standalone file.
     - Alternative: move it to `docs/agent/security_hardening.md` if you want all checklists together.
3. Update links across the repo to the new locations.
4. Delete `docs/standards/` after all links are updated.

## Files to touch
- `docs/standards/security_model.md` (move)
- `docs/standards/operations.md` (move)
- `docs/standards/docs_hygiene.md` (move or fold)
- `docs/standards/security_hardening.md` (merge or move)
- Link updates across `docs/` and `README.md`

## Acceptance criteria
- `docs/standards/` is removed from the repo.
- All internal links that previously referenced `docs/standards/*` resolve to the new locations.
- The moved docs are shorter than before and do not duplicate large sections of other canonical owners.
- No broken local links (including case-sensitive paths).

## Reduction
- Reduce folder and navigation surface area by eliminating a vague "standards" bucket.
- Reduce duplicate content by assigning clear owners.

## Suggested commit message
`docs: rehome standards docs into architecture`

