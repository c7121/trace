Architecture docs live here. Structure follows common product → spec → decisions flow.

## Layout
- `prd.md`: product requirements (who/why/what, goals, non-goals).
- `functional.md`: behaviors/flows, job model, security model, access control.
- `non-functional.md`: timeliness, integrity, reliability, scalability, cost, security, operations.
- `architecture.md`: system architecture, components, data model, infrastructure.
- `data_versioning.md`: incremental processing, reorg handling, staleness, deduplication.
- `testing.md`: BDD strategy, cucumber-rs setup, agent handoff format.
- `adr/`: architecture decision records (one file per decision).
- `operators/`: operator catalog (one file per operator implementation).
- `services/`: platform service specs (non-job components like query service).
- `diagrams/c4.md`: Mermaid C4 (System Context + Container + Component) viewable in Markdown preview.

## How to contribute
- Keep diagrams text-first (Mermaid/PlantUML). Link to sources if rendered elsewhere.
- Record owners, assumptions, and open questions in each doc.
- Use ADRs for discrete choices (e.g., orchestrator, IaC stack, secrets store, network posture).
