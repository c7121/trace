# Address Labels Data Model

Canonical DDL for the `address_labels` table.

> These tables live in **Postgres data**. Columns like `org_id`/`user_id` refer to entities in **Postgres state** and are **soft references** (no cross-DB foreign keys).

## address_labels

```sql
CREATE TABLE address_labels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL, -- soft ref: Postgres state orgs(id)
    user_id UUID NOT NULL, -- soft ref: Postgres state users(id)
    address TEXT NOT NULL,
    label TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'private',  -- see ../data_model/pii.md
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (org_id, user_id, address, label)
);

CREATE INDEX idx_address_labels_org ON address_labels(org_id);
CREATE INDEX idx_address_labels_user ON address_labels(user_id);
CREATE INDEX idx_address_labels_address ON address_labels(address);
```

## Related

- [address_labels.md](../operators/address_labels.md) — operator behavior and inputs
- [pii.md](pii.md) — visibility and audit rules