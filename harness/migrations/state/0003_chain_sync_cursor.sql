-- Lite chain sync planner progress tracking (Milestone 13).

-- High-water mark (next block to schedule) per chain.
CREATE TABLE IF NOT EXISTS state.chain_sync_cursor (
  chain_id    BIGINT PRIMARY KEY,
  next_block  BIGINT NOT NULL,
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Idempotent range scheduling ledger.
--
-- Ranges are inclusive and are scheduled exactly once per {chain_id, range_start, range_end}.
CREATE TABLE IF NOT EXISTS state.chain_sync_scheduled_ranges (
  chain_id     BIGINT NOT NULL,
  range_start  BIGINT NOT NULL,
  range_end    BIGINT NOT NULL,
  task_id      UUID NOT NULL,
  status       TEXT NOT NULL DEFAULT 'scheduled', -- scheduled|completed
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (chain_id, range_start, range_end),
  UNIQUE (task_id),
  CHECK (range_end >= range_start)
);

CREATE INDEX IF NOT EXISTS chain_sync_scheduled_ranges_status_idx
  ON state.chain_sync_scheduled_ranges (chain_id, status, range_start);
