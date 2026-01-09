-- Chain sync entrypoint durable state (Milestone 16).
--
-- This migration replaces the ms/13 single-stream cursor tables with job+stream scoped tables.
-- The ms/13 tables are renamed to `*_ms13` for debugging only.

-- Preserve ms/13 tables for reference; new implementation uses the v2 tables below.
ALTER TABLE IF EXISTS state.chain_sync_cursor RENAME TO chain_sync_cursor_ms13;
ALTER TABLE IF EXISTS state.chain_sync_scheduled_ranges RENAME TO chain_sync_scheduled_ranges_ms13;

-- Chain sync job definitions (apply/pause/resume/status).
CREATE TABLE IF NOT EXISTS state.chain_sync_jobs (
  job_id                      UUID PRIMARY KEY,
  org_id                      UUID NOT NULL,
  name                        TEXT NOT NULL,
  chain_id                    BIGINT NOT NULL,
  enabled                     BOOLEAN NOT NULL DEFAULT true,

  mode                        TEXT NOT NULL, -- fixed_target|follow_head
  from_block                  BIGINT NOT NULL,
  to_block                    BIGINT,        -- end-exclusive, required for fixed_target

  default_chunk_size          BIGINT NOT NULL,
  default_max_inflight        BIGINT NOT NULL,

  -- Follow-head settings (nullable for fixed_target).
  tail_lag                    BIGINT,
  head_poll_interval_seconds  INT,
  max_head_age_seconds        INT,

  -- Audit/change detector only; not a primary identity.
  yaml_hash                   TEXT NOT NULL,

  last_error_kind             TEXT,
  last_error_message          TEXT,
  created_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),

  UNIQUE (org_id, name),
  CHECK (chain_id > 0),
  CHECK (from_block >= 0),
  CHECK (default_chunk_size > 0),
  CHECK (default_max_inflight > 0)
);

CREATE INDEX IF NOT EXISTS chain_sync_jobs_enabled_idx
  ON state.chain_sync_jobs (org_id, enabled, updated_at);

-- Per-job dataset streams.
CREATE TABLE IF NOT EXISTS state.chain_sync_streams (
  job_id          UUID NOT NULL REFERENCES state.chain_sync_jobs(job_id) ON DELETE CASCADE,
  dataset_key     TEXT NOT NULL,
  cryo_dataset_name TEXT NOT NULL,
  rpc_pool        TEXT NOT NULL,
  config_hash     TEXT NOT NULL,

  -- Optional per-stream overrides.
  chunk_size      BIGINT,
  max_inflight    BIGINT,

  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

  PRIMARY KEY (job_id, dataset_key),
  CHECK (coalesce(chunk_size, 1) > 0),
  CHECK (coalesce(max_inflight, 1) > 0)
);

-- Per-stream high-water mark (next block to schedule), end-exclusive.
CREATE TABLE IF NOT EXISTS state.chain_sync_cursor (
  job_id      UUID NOT NULL REFERENCES state.chain_sync_jobs(job_id) ON DELETE CASCADE,
  dataset_key TEXT NOT NULL,
  next_block  BIGINT NOT NULL,
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

  PRIMARY KEY (job_id, dataset_key),
  CHECK (next_block >= 0)
);

CREATE INDEX IF NOT EXISTS chain_sync_cursor_updated_idx
  ON state.chain_sync_cursor (job_id, updated_at);

-- Idempotent range scheduling ledger.
--
-- Ranges are end-exclusive: `[range_start, range_end)`.
CREATE TABLE IF NOT EXISTS state.chain_sync_scheduled_ranges (
  job_id      UUID NOT NULL REFERENCES state.chain_sync_jobs(job_id) ON DELETE CASCADE,
  dataset_key TEXT NOT NULL,
  range_start BIGINT NOT NULL,
  range_end   BIGINT NOT NULL,
  task_id     UUID NOT NULL,
  status      TEXT NOT NULL DEFAULT 'scheduled', -- scheduled|completed
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

  PRIMARY KEY (job_id, dataset_key, range_start, range_end),
  UNIQUE (task_id),
  CHECK (range_end > range_start)
);

CREATE INDEX IF NOT EXISTS chain_sync_scheduled_ranges_status_idx
  ON state.chain_sync_scheduled_ranges (job_id, dataset_key, status, range_start);

