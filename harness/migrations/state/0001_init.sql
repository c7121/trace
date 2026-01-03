-- Postgres state (control-plane) minimal schema for contract-freeze harness.

CREATE SCHEMA IF NOT EXISTS state;

-- Task lifecycle (minimal)
CREATE TABLE IF NOT EXISTS state.tasks (
  task_id            UUID PRIMARY KEY,
  attempt            BIGINT NOT NULL DEFAULT 1,
  status             TEXT NOT NULL, -- queued|running|succeeded|failed|cancelled
  lease_token        UUID NULL,
  lease_expires_at   TIMESTAMPTZ NULL,
  payload            JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Outbox for durable side effects (queue publishes)
CREATE TABLE IF NOT EXISTS state.outbox (
  outbox_id          UUID PRIMARY KEY,
  topic              TEXT NOT NULL,
  payload            JSONB NOT NULL,
  available_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  attempts           INT NOT NULL DEFAULT 0,
  last_error         TEXT NULL,
  status             TEXT NOT NULL DEFAULT 'pending', -- pending|sent|dead
  created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A minimal pgqueue (wakeups + buffered dataset pointers)
CREATE TABLE IF NOT EXISTS state.queue_messages (
  message_id         UUID PRIMARY KEY,
  queue_name         TEXT NOT NULL,
  payload            JSONB NOT NULL,
  available_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  invisible_until    TIMESTAMPTZ NULL,
  deliveries         INT NOT NULL DEFAULT 0,
  created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS queue_messages_queue_available_idx
ON state.queue_messages (queue_name, available_at, created_at);

CREATE INDEX IF NOT EXISTS queue_messages_queue_invisible_idx
ON state.queue_messages (queue_name, invisible_until);
