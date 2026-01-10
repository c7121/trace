# address_labels

User-defined labels for blockchain addresses, stored as a joinable dataset.

Status: Planned

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
| `visibility` | user input | Visibility (see [pii.md](../../architecture/data_model/pii.md)) |

## Outputs

| Output | Location | Format |
|--------|----------|--------|
| Address labels | `postgres://address_labels` | Rows |

## PII Handling

PII column: `address_labels.label` (user-provided). Mark it as PII in dataset metadata; see [pii.md](../../architecture/data_model/pii.md) for visibility and audit rules.

## Example DAG Config

```yaml
- name: address_labels
  activation: source
  runtime: lambda
  operator: address_labels
  source:
    kind: manual
  outputs: 1
  update_strategy: replace
```

## Related

- [PII Handling](../../architecture/data_model/pii.md) - visibility rules, audit logging
