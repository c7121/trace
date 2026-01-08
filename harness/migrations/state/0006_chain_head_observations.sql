-- Chain head observations (optional input for follow-head planning).
--
-- This table is written by a trusted component (e.g., an RPC gateway) and read by the dispatcher
-- planner to compute the eligible end-exclusive planning window for follow-head chain sync jobs.

CREATE TABLE IF NOT EXISTS state.chain_head_observations (
  chain_id      BIGINT PRIMARY KEY,
  head_block    BIGINT NOT NULL,
  observed_at   TIMESTAMPTZ NOT NULL,
  source        TEXT,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

  CHECK (chain_id > 0),
  CHECK (head_block >= 0)
);

CREATE INDEX IF NOT EXISTS chain_head_observations_observed_idx
  ON state.chain_head_observations (chain_id, observed_at);

