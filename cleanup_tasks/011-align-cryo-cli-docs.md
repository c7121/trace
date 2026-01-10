# Cleanup Task 011: Align Cryo CLI notes with worker invocation

## Goal
Ensure docs reflect how the Cryo binary is invoked in the current `cryo_worker`.

## Why
`harness/NOTES.md` describes a Cryo CLI shape that does not match the current implementation in `harness/src/cryo_worker.rs`.

## Plan
- Update `harness/NOTES.md` to match the actual invocation:
  - `cryo <dataset> --rpc <url> --blocks <start:end> --output-dir <dir>`
- Keep the note short and point to the source of truth (`harness/src/cryo_worker.rs`) for the exact flags.

## Files to touch
- `harness/NOTES.md`

## Acceptance criteria
- No references to the old flag shape remain.
- The note matches what the worker actually runs.

## Suggested commit message
`docs: align Cryo CLI notes with cryo_worker`
