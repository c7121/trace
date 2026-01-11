# Address Labels Data Model

Schema mapping notes for Address Labels tables.

> These tables live in **Postgres data**. Columns like `org_id`/`user_id` refer to entities in **Postgres state** and are **soft references** (no cross-DB foreign keys).

Where to look:
- Columns: [data_schema.md](data_schema.md)

## data.address_labels (planned)

User-managed address label records.

- Invariants:
  - Uniqueness: `(org_id, user_id, address, label)`
  - Visibility uses the same levels as other user-managed datasets (see [pii.md](pii.md)).
- Expected indexes: `org_id`, `user_id`, `address`

## Related

- [address_labels.md](../operators/address_labels.md) - operator behavior and inputs
- [pii.md](pii.md) - visibility and audit rules
