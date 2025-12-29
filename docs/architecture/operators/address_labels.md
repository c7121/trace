# address_labels

User-defined labels for blockchain addresses, stored as a joinable dataset.

## Overview

| Property | Value |
|----------|-------|
| **Runtime** | `lambda` |
| **Activation** | `source` |
| **Source Kind** | `manual` |

## Description

Allows users to upload or define labels for blockchain addresses. Produces a dataset that can be joined with other datasets downstream (e.g., enrich transactions with address labels).

## Inputs

| Input | Type | Description |
|-------|------|-------------|
| `address` | user input | Blockchain address |
| `label` | user input | User-defined label |
| `visibility` | user input | Visibility (see [pii.md](../../capabilities/pii.md)) |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Address labels | `postgres://address_labels` | Rows |

## Schema

```sql
CREATE TABLE address_labels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES orgs(id),
    user_id UUID NOT NULL REFERENCES users(id),
    address TEXT NOT NULL,
    label TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'private',  -- see ../../capabilities/pii.md
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (org_id, user_id, address, label)
);

CREATE INDEX idx_address_labels_org ON address_labels(org_id);
CREATE INDEX idx_address_labels_user ON address_labels(user_id);
CREATE INDEX idx_address_labels_address ON address_labels(address);
```

## PII Handling

PII column: `address_labels.label` (user-provided). Mark it as PII in dataset metadata; see [pii.md](../../capabilities/pii.md) for visibility and audit rules.

## Example DAG Config

```yaml
- name: address_labels
  activation: source
  runtime: lambda
  operator: address_labels
  source:
    kind: manual
  output_datasets: [address_labels]
  update_strategy: replace
```

## Related

- [PII Handling](../../capabilities/pii.md) — visibility rules, audit logging
