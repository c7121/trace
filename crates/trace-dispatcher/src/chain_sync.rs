use anyhow::Context;
use serde::Deserialize;
use sqlx::{PgPool, Row};
use std::collections::BTreeMap;
use uuid::Uuid;

// UUIDv5 namespace for deterministic dataset UUIDs derived from {org_id, chain_id, dataset_key}.
const CHAIN_SYNC_DATASET_NAMESPACE: Uuid = Uuid::from_bytes([
    0x1f, 0x55, 0xc4, 0x95, 0x7e, 0x91, 0x44, 0x33, 0x8b, 0x4a, 0x0c, 0x08, 0x8c, 0x5a, 0x5c, 0x7f,
]);

// UUIDv5 namespace for a stable YAML change detector value.
const CHAIN_SYNC_YAML_NAMESPACE: Uuid = Uuid::from_bytes([
    0x41, 0x91, 0x5e, 0x64, 0x16, 0xd8, 0x4c, 0x48, 0x8f, 0x6c, 0xe4, 0x8a, 0x0f, 0x4b, 0x62, 0x0a,
]);

const DEFAULT_CHUNK_SIZE: i64 = 1_000;
const DEFAULT_MAX_INFLIGHT: i64 = 10;

#[derive(Debug, Clone)]
pub struct AppliedChainSyncJob {
    pub job_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainSyncYaml {
    kind: String,
    name: String,
    chain_id: i64,
    mode: ChainSyncMode,
    streams: BTreeMap<String, ChainSyncStream>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ChainSyncMode {
    FixedTarget {
        from_block: i64,
        to_block: i64,
    },
    FollowHead {
        from_block: i64,
        tail_lag: i64,
        head_poll_interval_seconds: i64,
        max_head_age_seconds: i64,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainSyncStream {
    cryo_dataset_name: String,
    rpc_pool: String,
    chunk_size: i64,
    max_inflight: i64,
}

pub async fn apply_chain_sync_yaml(
    pool: &PgPool,
    org_id: Uuid,
    yaml: &str,
) -> anyhow::Result<AppliedChainSyncJob> {
    let doc: ChainSyncYaml = serde_yaml::from_str(yaml).context("parse chain_sync yaml")?;
    validate_doc(&doc)?;

    let yaml_hash = Uuid::new_v5(&CHAIN_SYNC_YAML_NAMESPACE, yaml.as_bytes()).to_string();
    let job_name = doc.name.clone();

    let (mode, from_block, to_block, tail_lag, head_poll_interval_seconds, max_head_age_seconds) =
        match doc.mode {
            ChainSyncMode::FixedTarget {
                from_block,
                to_block,
            } => ("fixed_target", from_block, Some(to_block), None, None, None),
            ChainSyncMode::FollowHead {
                from_block,
                tail_lag,
                head_poll_interval_seconds,
                max_head_age_seconds,
            } => (
                "follow_head",
                from_block,
                None,
                Some(tail_lag),
                Some(head_poll_interval_seconds),
                Some(max_head_age_seconds),
            ),
        };

    let mut tx = pool.begin().await.context("begin chain_sync apply tx")?;

    let existing = sqlx::query(
        r#"
        SELECT job_id, enabled
        FROM state.chain_sync_jobs
        WHERE org_id = $1
          AND name = $2
        FOR UPDATE
        "#,
    )
    .bind(org_id)
    .bind(&job_name)
    .fetch_optional(&mut *tx)
    .await
    .context("lock existing chain sync job")?;

    let enabled = existing
        .as_ref()
        .map(|row| row.try_get::<bool, _>("enabled"))
        .transpose()
        .context("read enabled")?
        .unwrap_or(true);

    let job_id = existing
        .as_ref()
        .map(|row| row.try_get::<Uuid, _>("job_id"))
        .transpose()
        .context("read job_id")?
        .unwrap_or_else(Uuid::new_v4);

    let _ = sqlx::query(
        r#"
        INSERT INTO state.chain_sync_jobs (
          job_id,
          org_id,
          name,
          chain_id,
          enabled,
          mode,
          from_block,
          to_block,
          default_chunk_size,
          default_max_inflight,
          tail_lag,
          head_poll_interval_seconds,
          max_head_age_seconds,
          yaml_hash,
          last_error_kind,
          last_error_message,
          updated_at
        ) VALUES (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NULL, NULL, now()
        )
        ON CONFLICT (org_id, name) DO UPDATE SET
          chain_id = EXCLUDED.chain_id,
          mode = EXCLUDED.mode,
          from_block = EXCLUDED.from_block,
          to_block = EXCLUDED.to_block,
          default_chunk_size = EXCLUDED.default_chunk_size,
          default_max_inflight = EXCLUDED.default_max_inflight,
          tail_lag = EXCLUDED.tail_lag,
          head_poll_interval_seconds = EXCLUDED.head_poll_interval_seconds,
          max_head_age_seconds = EXCLUDED.max_head_age_seconds,
          yaml_hash = EXCLUDED.yaml_hash,
          last_error_kind = NULL,
          last_error_message = NULL,
          updated_at = now()
        RETURNING job_id
        "#,
    )
    .bind(job_id)
    .bind(org_id)
    .bind(&job_name)
    .bind(doc.chain_id)
    .bind(enabled)
    .bind(mode)
    .bind(from_block)
    .bind(to_block)
    .bind(DEFAULT_CHUNK_SIZE)
    .bind(DEFAULT_MAX_INFLIGHT)
    .bind(tail_lag)
    .bind(head_poll_interval_seconds)
    .bind(max_head_age_seconds)
    .bind(&yaml_hash)
    .fetch_one(&mut *tx)
    .await
    .context("upsert chain sync job")?
    .try_get::<Uuid, _>("job_id")
    .context("read upserted job_id")?;

    for (dataset_key, stream) in &doc.streams {
        validate_stream_key(dataset_key)?;
        validate_stream(stream)?;

        let config_hash = format!("cryo_ingest.{}:v1", stream.cryo_dataset_name);

        sqlx::query(
            r#"
            INSERT INTO state.chain_sync_streams (
              job_id,
              dataset_key,
              cryo_dataset_name,
              rpc_pool,
              config_hash,
              chunk_size,
              max_inflight,
              updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, now())
            ON CONFLICT (job_id, dataset_key) DO UPDATE SET
              cryo_dataset_name = EXCLUDED.cryo_dataset_name,
              rpc_pool = EXCLUDED.rpc_pool,
              config_hash = EXCLUDED.config_hash,
              chunk_size = EXCLUDED.chunk_size,
              max_inflight = EXCLUDED.max_inflight,
              updated_at = now()
            "#,
        )
        .bind(job_id)
        .bind(dataset_key)
        .bind(&stream.cryo_dataset_name)
        .bind(&stream.rpc_pool)
        .bind(&config_hash)
        .bind(stream.chunk_size)
        .bind(stream.max_inflight)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("upsert stream {dataset_key}"))?;

        sqlx::query(
            r#"
            INSERT INTO state.chain_sync_cursor (job_id, dataset_key, next_block)
            VALUES ($1, $2, $3)
            ON CONFLICT (job_id, dataset_key) DO NOTHING
            "#,
        )
        .bind(job_id)
        .bind(dataset_key)
        .bind(from_block)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("init cursor {dataset_key}"))?;

        sqlx::query(
            r#"
            UPDATE state.chain_sync_cursor
            SET next_block = GREATEST(next_block, $3),
                updated_at = now()
            WHERE job_id = $1
              AND dataset_key = $2
            "#,
        )
        .bind(job_id)
        .bind(dataset_key)
        .bind(from_block)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("ensure cursor >= from_block for {dataset_key}"))?;
    }

    tx.commit().await.context("commit chain_sync apply tx")?;
    Ok(AppliedChainSyncJob { job_id })
}

pub fn derive_dataset_uuid(org_id: Uuid, chain_id: i64, dataset_key: &str) -> anyhow::Result<Uuid> {
    anyhow::ensure!(chain_id > 0, "chain_id must be > 0");
    validate_stream_key(dataset_key)?;
    let name = format!("chain_sync:{org_id}:{chain_id}:{dataset_key}");
    Ok(Uuid::new_v5(&CHAIN_SYNC_DATASET_NAMESPACE, name.as_bytes()))
}

fn validate_doc(doc: &ChainSyncYaml) -> anyhow::Result<()> {
    anyhow::ensure!(doc.kind == "chain_sync", "kind must be chain_sync");
    anyhow::ensure!(!doc.name.trim().is_empty(), "name must not be empty");
    anyhow::ensure!(doc.chain_id > 0, "chain_id must be > 0");
    anyhow::ensure!(!doc.streams.is_empty(), "streams must not be empty");

    match doc.mode {
        ChainSyncMode::FixedTarget {
            from_block,
            to_block,
        } => {
            anyhow::ensure!(from_block >= 0, "from_block must be >= 0");
            anyhow::ensure!(to_block > from_block, "to_block must be > from_block");
        }
        ChainSyncMode::FollowHead {
            from_block,
            tail_lag,
            head_poll_interval_seconds,
            max_head_age_seconds,
        } => {
            anyhow::ensure!(from_block >= 0, "from_block must be >= 0");
            anyhow::ensure!(tail_lag >= 0, "tail_lag must be >= 0");
            anyhow::ensure!(
                head_poll_interval_seconds > 0,
                "head_poll_interval_seconds must be > 0"
            );
            anyhow::ensure!(max_head_age_seconds > 0, "max_head_age_seconds must be > 0");
        }
    }

    Ok(())
}

fn validate_stream_key(dataset_key: &str) -> anyhow::Result<()> {
    let ok = matches!(dataset_key, "blocks" | "logs" | "geth_logs" | "geth_calls");
    anyhow::ensure!(ok, "unsupported dataset_key: {dataset_key}");
    Ok(())
}

fn validate_stream(stream: &ChainSyncStream) -> anyhow::Result<()> {
    anyhow::ensure!(
        !stream.cryo_dataset_name.trim().is_empty(),
        "cryo_dataset_name must not be empty"
    );
    anyhow::ensure!(
        !stream.rpc_pool.trim().is_empty(),
        "rpc_pool must not be empty"
    );
    anyhow::ensure!(
        !stream.rpc_pool.contains("://"),
        "rpc_pool must be a name, not a URL"
    );
    anyhow::ensure!(stream.chunk_size > 0, "chunk_size must be > 0");
    anyhow::ensure!(stream.max_inflight > 0, "max_inflight must be > 0");
    Ok(())
}
