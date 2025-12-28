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

- All reads of PII columns are logged to `pii_access_log`
- Jobs must be explicitly granted access to PII datasets
- Jobs touching PII are tagged and subject to heightened audit/retention
- Hard delete only (GDPR compliance)

## PII Access Audit Log

```sql
CREATE TABLE pii_access_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    dataset TEXT NOT NULL,
    column_name TEXT,
    record_id UUID,
    action TEXT NOT NULL,  -- read, write, delete
    accessed_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_pii_access_log_user ON pii_access_log(user_id);
CREATE INDEX idx_pii_access_log_time ON pii_access_log(accessed_at);
CREATE INDEX idx_pii_access_log_dataset ON pii_access_log(dataset);
```

## Related

- [Security](../standards/security.md) — data access control model
- [Orchestration Data Model](orchestration.md) — users and org roles
- [address_labels operator](../architecture/operators/address_labels.md) — example dataset with PII
