# Setting up agent-standards sync (hardened)

## What this does
This repo vendors shared agent standards and keeps them updated via an automated weekly PR.

Shared files (synced):
- docs/agent/AGENTS.shared.md
- docs/agent/checklist.md
- docs/agent/references.md
- docs/specs/_mini_template.md
- docs/specs/_template.md
- docs/adr/_template.md

Repo-local overrides live in:
- AGENTS.md

## 1) Create the canonical repo
Create a repo like: YOUR_ORG/agent-standards
Put the shared files in the same relative paths as above.

## 2) Configure this repo's workflow
Edit:
- .github/workflows/sync-agent-standards.yml
and set:
- repository: YOUR_ORG/agent-standards
- ref: main (or a tag/branch you prefer)

## 3) If the upstream is private
Create a repo secret:
- AGENT_STANDARDS_TOKEN
that can read the upstream repo contents.

## 4) Action hardening notes
This workflow pins GitHub Actions by full commit SHA (instead of tags) to reduce supply-chain risk.
Periodically update pinned SHAs as part of your normal dependency hygiene.

## 5) Allow workflows to create PRs
In GitHub repo settings:
- Settings → Actions → General → Workflow permissions
  - set to "Read and write permissions"
  - and allow GitHub Actions to create pull requests (org/enterprise settings may also apply)

Then either wait for the scheduled run or trigger it manually via Actions → "Sync agent standards".

## Notes on TypeScript execution
The sync script is TypeScript (.cts) and runs directly with Node.js.
Node executes erasable TypeScript syntax via type stripping (no type-checking).
