-- Postgres data (data-plane) minimal sink schema for contract-freeze harness.

CREATE SCHEMA IF NOT EXISTS data;

-- Alert events sink (strict schema, idempotent insert via dedupe_key)
CREATE TABLE IF NOT EXISTS data.alert_events (
  dedupe_key           TEXT PRIMARY KEY,
  alert_definition_id  UUID NOT NULL,
  event_time           TIMESTAMPTZ NOT NULL,

  -- Optional but commonly indexed chain context (v1)
  chain_id             BIGINT NULL,
  block_number         BIGINT NULL,
  block_hash           TEXT NULL,
  tx_hash              TEXT NULL,

  payload              JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

