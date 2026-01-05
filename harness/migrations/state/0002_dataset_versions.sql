-- Dataset versions registry (Lite harness).
--
-- This is a minimal state DB table for Milestone 12: Cryo local sync worker.
-- It records version-addressed Parquet dataset artifacts written to object storage.

CREATE TABLE IF NOT EXISTS state.dataset_versions (
  dataset_version   UUID PRIMARY KEY,
  dataset_uuid      UUID NOT NULL,
  storage_prefix    TEXT NOT NULL, -- version-addressed `s3://bucket/prefix/` containing `_manifest.json`
  config_hash       TEXT NOT NULL, -- stable hash of config + chain + dataset kind
  range_start       BIGINT NOT NULL,
  range_end         BIGINT NOT NULL,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Determinism invariant: one published dataset version per {dataset_uuid, config_hash, range}.
CREATE UNIQUE INDEX IF NOT EXISTS dataset_versions_determinism_idx
ON state.dataset_versions (dataset_uuid, config_hash, range_start, range_end);

