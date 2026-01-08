use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

pub async fn head_observer_tick_once(
    pool: &PgPool,
    client: &reqwest::Client,
) -> anyhow::Result<i64> {
    let rows = sqlx::query(
        r#"
        SELECT
          j.org_id,
          j.chain_id,
          s.rpc_pool,
          MIN(j.head_poll_interval_seconds)::bigint AS poll_interval_seconds
        FROM state.chain_sync_jobs j
        JOIN state.chain_sync_streams s
          ON s.job_id = j.job_id
        WHERE j.enabled = true
          AND j.mode = 'follow_head'
        GROUP BY j.org_id, j.chain_id, s.rpc_pool
        "#,
    )
    .fetch_all(pool)
    .await
    .context("select follow_head rpc pools")?;

    let mut updated = 0i64;
    for row in rows {
        let org_id: Uuid = row.try_get("org_id").context("org_id")?;
        let chain_id: i64 = row.try_get("chain_id").context("chain_id")?;
        let rpc_pool: String = row.try_get("rpc_pool").context("rpc_pool")?;
        let poll_interval_seconds: Option<i64> = row
            .try_get("poll_interval_seconds")
            .context("poll_interval_seconds")?;

        let poll_interval_seconds = poll_interval_seconds.unwrap_or(0).max(0);
        let poll_interval = chrono::Duration::seconds(poll_interval_seconds);

        let existing_observed_at: Option<DateTime<Utc>> = sqlx::query_scalar(
            r#"
            SELECT observed_at
            FROM state.chain_head_observations
            WHERE org_id = $1
              AND chain_id = $2
              AND rpc_pool = $3
            "#,
        )
        .bind(org_id)
        .bind(chain_id)
        .bind(&rpc_pool)
        .fetch_optional(pool)
        .await
        .context("select existing chain_head_observations")?;

        let now = Utc::now();
        if let Some(observed_at) = existing_observed_at {
            if poll_interval_seconds > 0 && observed_at + poll_interval > now {
                continue;
            }
        }

        let Some(rpc_url) = rpc_url_for_pool(&rpc_pool) else {
            tracing::warn!(
                event = "trace.dispatcher.chain_head_observer.missing_rpc_url",
                org_id = %org_id,
                chain_id,
                rpc_pool = %rpc_pool,
                "missing RPC URL; set TRACE_RPC_POOL_<NAME>_URL"
            );
            continue;
        };

        let head_block = match fetch_eth_block_number(client, &rpc_url).await {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!(
                    event = "trace.dispatcher.chain_head_observer.rpc_error",
                    org_id = %org_id,
                    chain_id,
                    rpc_pool = %rpc_pool,
                    error = %err,
                    "failed to fetch eth_blockNumber"
                );
                continue;
            }
        };

        sqlx::query(
            r#"
            INSERT INTO state.chain_head_observations (
              org_id, chain_id, rpc_pool, head_block, observed_at, source, updated_at
            ) VALUES (
              $1, $2, $3, $4, $5, $6, now()
            )
            ON CONFLICT (org_id, chain_id, rpc_pool) DO UPDATE SET
              head_block = EXCLUDED.head_block,
              observed_at = EXCLUDED.observed_at,
              source = EXCLUDED.source,
              updated_at = now()
            "#,
        )
        .bind(org_id)
        .bind(chain_id)
        .bind(&rpc_pool)
        .bind(head_block)
        .bind(now)
        .bind(&rpc_pool)
        .execute(pool)
        .await
        .context("upsert chain_head_observations")?;

        updated += 1;
    }

    Ok(updated)
}

fn rpc_url_for_pool(pool: &str) -> Option<String> {
    let pool = pool.trim();
    if pool.is_empty() {
        return None;
    }

    let mut key = String::with_capacity(pool.len());
    for c in pool.chars() {
        if c.is_ascii_alphanumeric() {
            key.push(c.to_ascii_uppercase());
        } else {
            key.push('_');
        }
    }

    let env_key = format!("TRACE_RPC_POOL_{key}_URL");
    std::env::var(env_key).ok()
}

async fn fetch_eth_block_number(client: &reqwest::Client, rpc_url: &str) -> anyhow::Result<i64> {
    let resp = client
        .post(rpc_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_blockNumber",
            "params": [],
        }))
        .send()
        .await
        .context("send eth_blockNumber")?;

    let status = resp.status();
    let body: JsonRpcResponse<String> = resp.json().await.context("decode json-rpc response")?;
    if !status.is_success() {
        anyhow::bail!("rpc status {status}");
    }

    if let Some(err) = body.error {
        anyhow::bail!("rpc error {}: {}", err.code, err.message);
    }

    let Some(result) = body.result else {
        anyhow::bail!("missing json-rpc result");
    };

    parse_hex_u64(&result)
        .context("parse eth_blockNumber hex")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("eth_blockNumber overflow"))
}

fn parse_hex_u64(s: &str) -> anyhow::Result<u64> {
    let s = s.trim();
    let s = s.strip_prefix("0x").unwrap_or(s);
    u64::from_str_radix(s, 16).context("from_str_radix")
}
