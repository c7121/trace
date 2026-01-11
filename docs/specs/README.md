# Specs

Specs define Trace behavior surfaces (JTBD): what the system does, what inputs and outputs mean, and what must be true at boundaries. When changing behavior, start here and ensure changes remain consistent with system invariants and interface contracts.

## Governance

Canonical rules for when to write a spec and which template to use live in [docs/agent/AGENTS.shared.md](../agent/AGENTS.shared.md).

Minimum workflow for any spec:
- Declare `Risk` (Low/Medium/High).
- Declare `Public surface` changes (endpoints, payloads, config, migrations, persistence formats).
- Link to the relevant owners in `docs/architecture/*` instead of restating contracts.

## Index

- Platform configuration and operators:
  - DAG YAML surface: [dag_configuration.md](dag_configuration.md)
  - Operator surfaces: [operators/README.md](operators/README.md)
- Chain sync and ingestion:
  - Chain sync entrypoint: [chain_sync_entrypoint.md](chain_sync_entrypoint.md)
  - Ingestion patterns: [ingestion.md](ingestion.md)
  - Cryo library adapter spike (non-normative): [cryo_library_adapter.md](cryo_library_adapter.md)
- Query:
  - User query API: [query_service_user_query.md](query_service_user_query.md)
  - Task query API: [query_service_task_query.md](query_service_task_query.md)
  - Query results: [query_service_query_results.md](query_service_query_results.md)
  - SQL safety gate: [query_sql_gating.md](query_sql_gating.md)
- UDF:
  - UDF model: [udf.md](udf.md)
  - Bundle manifest: [udf_bundle_manifest.md](udf_bundle_manifest.md)
- Alerting:
  - Alerting model: [alerting.md](alerting.md)
- Cross-cutting:
  - Metadata surfaces: [metadata.md](metadata.md)
  - Error contract: [trace_core_error_contract.md](trace_core_error_contract.md)

## Related (architecture and decisions)

- Architecture index: [../architecture/README.md](../architecture/README.md)
- System invariants: [../architecture/invariants.md](../architecture/invariants.md)
- Interface contracts: [../architecture/contracts.md](../architecture/contracts.md)
- Security model: [../architecture/security.md](../architecture/security.md)
- ADRs: [../adr/README.md](../adr/README.md)

## Templates

- Mini spec template: [_mini_template.md](_mini_template.md)
- Full spec template: [_template.md](_template.md)
