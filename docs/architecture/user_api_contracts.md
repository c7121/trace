# User API contracts

This document enumerates the **user-facing** (`/v1/*`) routes that are reachable via the Gateway.
It exists to reduce surface area: if it is not listed here, it is **not** a user API.

This document does **not** restate payload schemas in full; feature specs under `docs/specs/` own the detailed shapes.

## Scope

- **Included:** user-facing routes called by end users and control-plane tooling.
- **Excluded:** task-scoped routes used by runners/UDFs (`/v1/task/*`) and all `/internal/*` routes.

## Authn / authz invariants

- All user routes require `Authorization: Bearer <user_jwt>`.
- The Gateway (or the service behind it) MUST map the user JWT into an identity context:
  - `org_id` (tenant boundary)
  - `user_id` (actor id)
  - roles/permissions (if enabled)
- All reads and writes MUST be scoped to `org_id`. Cross-org access is always forbidden.

Claim mapping for the user JWT is defined in: `docs/standards/security_model.md`.

## Route allowlist

### Query

No user-facing query routes are implemented yet.

### Datasets (discovery)

- `GET /v1/datasets` - list published datasets (names + metadata).
- `GET /v1/datasets/{dataset_name}` - fetch dataset metadata and current version pointer.

Owned by: Dispatcher (registry is in Postgres state).
References: ADR `0008-dataset-registry-and-publishing.md`, `docs/architecture/data_model/orchestration.md`.

### DAG deployment

- `POST /v1/dags/{dag_name}/versions` - deploy a DAG YAML (idempotent by `yaml_hash`).
- `GET /v1/dags/{dag_name}/versions` - list deployed versions.
- `PUT /v1/dags/{dag_name}/active` - set the org+dag active pointer to a deployed version.

Spec: `docs/specs/dag_configuration.md`.

### UDF bundles

UDF bundle upload is a two-step control-plane flow to keep large payloads out of the Gateway.

- `POST /v1/udf/bundles` - create an upload session and return a pre-signed upload URL.
- `POST /v1/udf/bundles/{bundle_id}/finalize` - finalize metadata (content hash, runtime, entrypoint).

Spec: `docs/specs/udf.md`.

### Alerts

- `POST /v1/alerts` - create an alert definition.
- `GET /v1/alerts` - list alerts.
- `GET /v1/alerts/{alert_id}` - fetch an alert definition.
- `PUT /v1/alerts/{alert_id}` - update an alert definition.
- `DELETE /v1/alerts/{alert_id}` - disable/delete an alert definition.
- `GET /v1/alerts/{alert_id}/deliveries` - list delivery attempts/status.

Spec: `docs/specs/alerting.md`.

## Explicit non-user routes

The following are intentionally **not** user-facing:

- `/v1/task/*` - runner/UDF-only task-scoped APIs (capability token gated).
- `/internal/*` - internal-only component APIs.

## Future (not in allowlist)

The following are planned but are **not** reachable via the Gateway today:

- `POST /v1/query` - user-facing interactive query execution (Query Service).
  - Blocked on dataset registry + published dataset authz and a stable query results model.
  - Reference: `docs/architecture/containers/query_service.md`
