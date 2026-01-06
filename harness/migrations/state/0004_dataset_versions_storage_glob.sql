-- Dataset version storage references (prefix + glob).
--
-- Query Service attaches Parquet datasets by a trusted storage reference carried in the task
-- capability token. For Lite/harness this is typically `s3://{bucket}/{prefix}` + `*.parquet`.

ALTER TABLE state.dataset_versions
ADD COLUMN IF NOT EXISTS storage_glob TEXT NOT NULL DEFAULT '*.parquet';

