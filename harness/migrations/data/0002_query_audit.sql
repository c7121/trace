-- Query audit log (dataset-level only; no raw SQL stored).

CREATE TABLE IF NOT EXISTS data.query_audit (
  id               BIGSERIAL PRIMARY KEY,
  org_id           UUID NOT NULL,
  task_id          UUID NOT NULL,
  dataset_id       UUID NOT NULL,
  query_time       TIMESTAMPTZ NOT NULL DEFAULT now(),
  columns_accessed JSONB NULL,
  result_row_count BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS query_audit_org_time_idx
  ON data.query_audit (org_id, query_time DESC);

CREATE INDEX IF NOT EXISTS query_audit_task_time_idx
  ON data.query_audit (task_id, query_time DESC);

