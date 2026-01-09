-- User query audit log (dataset-level only; no raw SQL stored).

CREATE TABLE IF NOT EXISTS data.user_query_audit (
  id               BIGSERIAL PRIMARY KEY,
  org_id           UUID NOT NULL,
  user_sub         TEXT NOT NULL,
  dataset_id       UUID NOT NULL,
  query_time       TIMESTAMPTZ NOT NULL DEFAULT now(),
  columns_accessed JSONB NULL,
  result_row_count BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS user_query_audit_org_time_idx
  ON data.user_query_audit (org_id, query_time DESC);

CREATE INDEX IF NOT EXISTS user_query_audit_user_time_idx
  ON data.user_query_audit (user_sub, query_time DESC);

