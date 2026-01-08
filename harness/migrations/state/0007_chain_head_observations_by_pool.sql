-- Chain head observations (input for follow-head planning), scoped by org and RPC pool.
--
-- This replaces the earlier chain_id-only table so follow-head planning can be bounded by the
-- same rpc_pool that will execute each dataset stream.

ALTER TABLE IF EXISTS state.chain_head_observations RENAME TO chain_head_observations_ms16;

CREATE TABLE IF NOT EXISTS state.chain_head_observations (
  org_id       UUID NOT NULL,
  chain_id     BIGINT NOT NULL,
  rpc_pool     TEXT NOT NULL,
  head_block   BIGINT NOT NULL,
  observed_at  TIMESTAMPTZ NOT NULL,
  source       TEXT,
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),

  PRIMARY KEY (org_id, chain_id, rpc_pool),
  CHECK (chain_id > 0),
  CHECK (head_block >= 0)
);

CREATE INDEX IF NOT EXISTS chain_head_observations_observed_idx
  ON state.chain_head_observations (org_id, chain_id, rpc_pool, observed_at);

