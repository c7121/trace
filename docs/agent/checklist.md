# Agent preflight checklist

Use this checklist before coding and again before claiming “done”.

- [ ] Read shared standards: docs/agent/AGENTS.shared.md (and repo-local overrides in AGENTS.md)
- [ ] Doc chosen: mini spec (docs/specs/_mini_template.md) OR full spec (docs/specs/_template.md)
- [ ] Mini spec limits (if used): <= 120 words, <= 5 bullets, no diagrams/tables/extra sections
- [ ] Spec-first: spec exists/updated before code (and ADR exists if decision changes long-term constraints)
- [ ] Risk declared: Risk is set (Low/Medium/High). If High: full spec + monitoring + rollback explicit; approval requested before implementation
- [ ] Public surface declared: any new/changed endpoints/events/CLI/config/persistence/entrypoint exports are listed (or “None”)
- [ ] No smuggling: no new public behavior is hidden inside existing surfaces without being declared
- [ ] C4 anti-drift (full specs): if boundaries/data flow changed, Mermaid-in-Markdown diagram updated (no box soup; labeled relationships)
- [ ] Reduction pass: simplification considered. If none viable without changing requirements/compatibility, say why
- [ ] Security bar: inputs validated at boundaries; authz explicit; secrets/PII not logged; resource limits considered
- [ ] Supply chain: dependency changes are justified; prefer minimal deps; for Rust consider RustSec/cargo-audit/cargo-deny/cargo-vet
- [ ] Idiomatic + local cleanup: touched code is idiomatic; touched-area non-idioms cleaned up when it reduces complexity
- [ ] Anchor check: any cited reference was actually consulted and tied to a specific claim/decision; non-trivial claims are anchored to in-repo examples (path) or docs/agent/references.md
- [ ] No prod actions: no production actions were taken unless explicitly instructed and confirmed
- [ ] Execution + tests: agent proposes exact commands (asks before running). Acceptance criteria map to tests; avoid redundant tests
