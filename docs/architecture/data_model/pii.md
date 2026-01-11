# PII Handling

Policy and mechanisms for managing personally identifiable information (PII) in datasets.

## Overview

PII is a **column-level classification**. Any dataset can have columns marked as PII. The system enforces access controls and audit logging for PII-classified data.

## Marking Columns as PII

Columns are classified as PII in dataset metadata.

Use `pii_columns` to explicitly list PII columns by name:

```yaml
datasets:
  address_labels:
    pii_columns: [label]        # user-provided text
  saved_queries:
    pii_columns: [query]        # user-provided text
  alert_definitions:
    pii_columns: [channels]     # may contain email/phone/webhook URLs
```

When documenting a dataset/operator, call out known PII columns in the form
`<dataset>.<column>` and include a short note to mark them as PII in dataset metadata.

## Visibility Rules

Datasets or rows containing PII support visibility levels:

- `private`: only creator can read
- `org`: any org member can read
- `role:<role_slug>`: any member of that org-defined role (e.g., `role:finance`)
- `public`: anyone can read (future)

## Access Rules

- All reads of PII-classified data through platform APIs are logged to `pii_access_log`.
  - When the platform can reliably attribute column access, populate `column_name`.
  - For arbitrary SQL (Query Service), log dataset-level access and leave `column_name` as `NULL`.
- Jobs must be explicitly granted access to PII datasets.
- Jobs touching PII are tagged and subject to heightened audit/retention.
- Hard delete only (GDPR compliance).

## PII Access Audit Log

> `pii_access_log` lives in **Postgres data** for auditing. `org_id`/`user_id`/`task_id` are **soft references**
> to entities in Postgres state (no cross-DB foreign keys).
>
> Exactly one principal is set per row: either `user_id` (for user API reads/writes) or `task_id` (for task-scoped reads/writes).

Where to look:
- Columns: [data_schema.md](data_schema.md)

Invariants:
- Exactly one principal is set per row: `user_id` XOR `task_id`.
- `org_id` is always set for audit attribution and retention.
- `column_name` may be `NULL` when column-level attribution is not possible (for example Query Service SQL).
- Expected indexes: `org_id`, `user_id`, `task_id`, `accessed_at`, `dataset`

## Related

- [Security](../security.md) - data access control model
- [Orchestration Data Model](orchestration.md) - users and org roles
- [address_labels operator](../operators/address_labels.md) - example dataset with PII
