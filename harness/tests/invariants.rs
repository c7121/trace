use anyhow::Context;
use duckdb::Connection;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Once, OnceLock},
    time::Duration,
};
use trace_core::{
    fixtures::{
        ALERTS_FIXTURE_DATASET_ID, ALERTS_FIXTURE_DATASET_STORAGE_PREFIX,
        ALERTS_FIXTURE_DATASET_VERSION,
    },
    manifest::DatasetManifestV1,
    runtime::RuntimeInvoker,
    udf::UdfInvocationPayload,
    DatasetGrant, ObjectStore as ObjectStoreTrait, S3Grants, Signer as SignerTrait,
    TaskCapabilityIssueRequest,
};
use trace_dispatcher::chain_sync::{apply_chain_sync_yaml, derive_dataset_uuid};
use trace_harness::constants::{
    CONTENT_TYPE_JSON, CONTENT_TYPE_JSONL, DEFAULT_ALERT_DEFINITION_ID, TASK_CAPABILITY_HEADER,
};
use trace_harness::jwt::{Hs256TaskCapabilityConfig, TaskCapability};
use trace_harness::{
    config::HarnessConfig,
    cryo_worker::{derive_dataset_publication, CryoIngestPayload},
    dispatcher::DispatcherServer,
    dispatcher_client::DispatcherClient,
    migrate,
    pgqueue::PgQueue,
    runner::FakeRunner,
    s3::{parse_s3_uri, ObjectStore},
};
use trace_query_service::{
    build_state as build_query_state, config::QueryServiceConfig, router as query_router,
    TaskQueryRequest,
};
use uuid::Uuid;

fn unique_queue(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4())
}

fn init_tracing() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt::try_init();
    });
}

async fn integration_lock() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
}

fn ensure_duckdb_httpfs_installed() -> anyhow::Result<()> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    let res = INIT.get_or_init(|| {
        (|| -> anyhow::Result<()> {
            let mut ext_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            ext_dir.push("target");
            ext_dir.push("duckdb_extensions");
            std::fs::create_dir_all(&ext_dir).context("create duckdb extension dir")?;

            std::env::set_var("DUCKDB_EXTENSION_DIRECTORY", &ext_dir);

            let conn = Connection::open_in_memory().context("open duckdb in-memory")?;
            conn.execute_batch("INSTALL httpfs;")
                .context("INSTALL httpfs extension")?;
            Ok(())
        })()
        .map_err(|err| format!("{err:#}"))
    });

    match res {
        Ok(()) => Ok(()),
        Err(msg) => Err(anyhow::anyhow!(msg.clone())),
    }
}

async fn migrated_config() -> anyhow::Result<HarnessConfig> {
    init_tracing();
    ensure_duckdb_httpfs_installed()?;
    std::env::set_var("TRACE_CRYO_MODE", "fake");

    let mut cfg = HarnessConfig::from_env().context("load harness config")?;
    cfg.task_wakeup_queue = unique_queue("task_wakeup_test");
    cfg.buffer_queue = unique_queue("buffer_queue_test");
    cfg.buffer_queue_dlq = unique_queue("buffer_queue_dlq_test");
    migrate::run(&cfg).await.context("run migrations")?;

    // Tests share the same Postgres databases. Clean state between tests to avoid cross-test
    // planner/outbox interference (jobs persist in tables, but queue names change per test).
    let state_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db for cleanup")?;
    sqlx::query(
        r#"
        TRUNCATE
          state.tasks,
          state.outbox,
          state.queue_messages,
          state.dataset_versions,
          state.chain_sync_jobs,
          state.chain_sync_cursor,
          state.chain_sync_scheduled_ranges,
          state.chain_head_observations,
          state.chain_sync_cursor_ms13,
          state.chain_sync_scheduled_ranges_ms13,
          state.chain_head_observations_ms16
        RESTART IDENTITY
        CASCADE
        "#,
    )
    .execute(&state_pool)
    .await
    .context("truncate state tables")?;

    let data_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&cfg.data_database_url)
        .await
        .context("connect data db for cleanup")?;
    sqlx::query(
        r#"
        TRUNCATE
          data.alert_events,
          data.query_audit,
          data.user_query_audit
        RESTART IDENTITY
        "#,
    )
    .execute(&data_pool)
    .await
    .context("truncate data tables")?;
    Ok(cfg)
}

const CONTENT_TYPE_PARQUET: &str = "application/octet-stream";

fn join_key(prefix: &str, leaf: &str) -> String {
    let prefix = prefix.trim_end_matches('/');
    format!("{prefix}/{leaf}")
}

async fn build_alerts_fixture_parquet_bytes() -> anyhow::Result<Vec<u8>> {
    tokio::task::spawn_blocking(|| {
        let dir = std::env::temp_dir().join(format!("trace-harness-fixture-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).context("create temp dir")?;
        let parquet_path = dir.join("alerts_fixture.parquet");

        let conn = Connection::open_in_memory().context("open duckdb in-memory")?;
        conn.execute_batch(
            r#"
            BEGIN;
            CREATE TABLE alerts_fixture (
              alert_definition_id VARCHAR NOT NULL,
              dedupe_key          VARCHAR NOT NULL,
              event_time          TIMESTAMP NOT NULL,
              chain_id            BIGINT NOT NULL,
              block_number        BIGINT NOT NULL,
              block_hash          VARCHAR NOT NULL,
              tx_hash             VARCHAR NOT NULL,
              payload             VARCHAR NOT NULL
            );

            INSERT INTO alerts_fixture VALUES
              (
                'alert_definition_1',
                'dedupe-001',
                '2025-01-01T00:00:00Z',
                1,
                100,
                '0xblockhash001',
                '0xtxhash001',
                '{"severity":"low","msg":"fixture-1"}'
              ),
              (
                'alert_definition_1',
                'dedupe-002',
                '2025-01-01T00:00:01Z',
                1,
                101,
                '0xblockhash002',
                '0xtxhash002',
                '{"severity":"medium","msg":"fixture-2"}'
              ),
              (
                'alert_definition_2',
                'dedupe-003',
                '2025-01-01T00:00:02Z',
                10,
                202,
                '0xblockhash003',
                '0xtxhash003',
                '{"severity":"high","msg":"fixture-3"}'
              );
            COMMIT;
            "#,
        )
        .context("create fixture table")?;

        let parquet_escaped = parquet_path.to_string_lossy().replace('\'', "''");
        conn.execute_batch(&format!(
            "COPY alerts_fixture TO '{parquet_escaped}' (FORMAT PARQUET);"
        ))
        .context("copy to parquet")?;

        let bytes = std::fs::read(&parquet_path).context("read parquet bytes")?;
        let _ = std::fs::remove_dir_all(&dir);
        Ok::<_, anyhow::Error>(bytes)
    })
    .await
    .context("join parquet builder")?
}

async fn seed_alerts_fixture_dataset(cfg: &HarnessConfig) -> anyhow::Result<()> {
    let object_store = ObjectStore::new(&cfg.s3_endpoint)?;
    let (bucket, prefix_key) =
        parse_s3_uri(ALERTS_FIXTURE_DATASET_STORAGE_PREFIX).context("parse storage prefix")?;

    let parquet_bytes = build_alerts_fixture_parquet_bytes().await?;
    let parquet_key = join_key(&prefix_key, "alerts_fixture.parquet");
    object_store
        .put_bytes(&bucket, &parquet_key, parquet_bytes, CONTENT_TYPE_PARQUET)
        .await?;

    let manifest_key = join_key(&prefix_key, "_manifest.json");
    let manifest = DatasetManifestV1 {
        version: DatasetManifestV1::VERSION,
        dataset_uuid: ALERTS_FIXTURE_DATASET_ID,
        dataset_version: ALERTS_FIXTURE_DATASET_VERSION,
        parquet_keys: vec![parquet_key],
    };
    let manifest_bytes = serde_json::to_vec(&manifest).context("encode fixture manifest")?;
    object_store
        .put_bytes(&bucket, &manifest_key, manifest_bytes, CONTENT_TYPE_JSON)
        .await?;

    Ok(())
}

#[tokio::test]
async fn duplicate_claims_do_not_double_run() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let cfg = migrated_config().await?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let server = DispatcherServer::start(
        pool,
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        false,
        false,
        false,
    )
    .await?;

    let base = format!("http://{}", server.addr);
    let client = reqwest::Client::new();
    let task_id = Uuid::new_v4();

    let (r1, r2) = tokio::join!(
        client
            .post(format!("{base}/internal/task-claim"))
            .json(&serde_json::json!({ "task_id": task_id }))
            .send(),
        client
            .post(format!("{base}/internal/task-claim"))
            .json(&serde_json::json!({ "task_id": task_id }))
            .send()
    );

    let s1 = r1?.status();
    let s2 = r2?.status();
    let ok = [s1, s2].iter().filter(|s| s.is_success()).count();
    let conflict = [s1, s2]
        .iter()
        .filter(|s| **s == reqwest::StatusCode::CONFLICT)
        .count();

    anyhow::ensure!(
        ok == 1 && conflict == 1,
        "expected one 200 and one 409, got {s1} and {s2}"
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn capability_token_required() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let cfg = migrated_config().await?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let server = DispatcherServer::start(
        pool,
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        false,
        false,
        false,
    )
    .await?;

    let base = format!("http://{}", server.addr);
    let client = reqwest::Client::new();
    let task_id = Uuid::new_v4();

    let claim = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let attempt = claim["attempt"].as_i64().context("attempt")?;
    let lease_token = claim["lease_token"].as_str().context("lease_token")?;

    let resp = client
        .post(format!("{base}/v1/task/heartbeat"))
        .json(&serde_json::json!({
            "task_id": task_id,
            "attempt": attempt,
            "lease_token": lease_token,
        }))
        .send()
        .await?;

    anyhow::ensure!(
        resp.status() == reqwest::StatusCode::UNAUTHORIZED,
        "expected 401, got {}",
        resp.status()
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn wrong_capability_token_rejected() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let cfg = migrated_config().await?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let server = DispatcherServer::start(
        pool,
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        false,
        false,
        false,
    )
    .await?;

    let base = format!("http://{}", server.addr);
    let client = reqwest::Client::new();

    let task_a = Uuid::new_v4();
    let task_b = Uuid::new_v4();

    let claim_a = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_a }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let claim_b = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_b }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let attempt_b = claim_b["attempt"].as_i64().context("attempt_b")?;
    let lease_token_b = claim_b["lease_token"].as_str().context("lease_token_b")?;
    let token_a = claim_a["capability_token"].as_str().context("token_a")?;

    let resp = client
        .post(format!("{base}/v1/task/heartbeat"))
        .header(TASK_CAPABILITY_HEADER, token_a)
        .json(&serde_json::json!({
            "task_id": task_b,
            "attempt": attempt_b,
            "lease_token": lease_token_b,
        }))
        .send()
        .await?;

    anyhow::ensure!(
        resp.status() == reqwest::StatusCode::FORBIDDEN,
        "expected 403, got {}",
        resp.status()
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn wrong_lease_token_rejected() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let cfg = migrated_config().await?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let server = DispatcherServer::start(
        pool,
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        false,
        false,
        false,
    )
    .await?;

    let base = format!("http://{}", server.addr);
    let client = reqwest::Client::new();
    let task_id = Uuid::new_v4();

    let claim = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let attempt = claim["attempt"].as_i64().context("attempt")?;
    let token = claim["capability_token"]
        .as_str()
        .context("capability_token")?;

    let resp = client
        .post(format!("{base}/v1/task/heartbeat"))
        .header(TASK_CAPABILITY_HEADER, token)
        .json(&serde_json::json!({
            "task_id": task_id,
            "attempt": attempt,
            "lease_token": Uuid::new_v4(),
        }))
        .send()
        .await?;

    anyhow::ensure!(
        resp.status() == reqwest::StatusCode::CONFLICT,
        "expected 409, got {}",
        resp.status()
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn next_key_token_accepted_during_overlap() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let mut cfg = migrated_config().await?;
    cfg.task_capability_kid = "current".to_string();
    cfg.task_capability_secret = "current-secret".to_string();
    cfg.task_capability_next_kid = Some("next".to_string());
    cfg.task_capability_next_secret = Some("next-secret".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let server = DispatcherServer::start(
        pool,
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        false,
        false,
        false,
    )
    .await?;

    let base = format!("http://{}", server.addr);
    let client = reqwest::Client::new();
    let task_id = Uuid::new_v4();

    let claim = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let attempt = claim["attempt"].as_i64().context("attempt")?;
    let lease_token = claim["lease_token"].as_str().context("lease_token")?;

    let now = chrono::Utc::now().timestamp();
    let iat: usize = now.try_into().unwrap_or(0);
    let exp: usize = (now + 60).try_into().unwrap_or(usize::MAX);
    let claims = trace_harness::jwt::TaskCapabilityClaims {
        iss: cfg.task_capability_iss.clone(),
        aud: cfg.task_capability_aud.clone(),
        sub: format!("task:{task_id}"),
        exp,
        iat,
        org_id: cfg.org_id,
        task_id,
        attempt,
        datasets: Vec::new(),
        s3: trace_harness::jwt::S3Grants {
            read_prefixes: Vec::new(),
            write_prefixes: Vec::new(),
        },
    };

    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    header.kid = cfg.task_capability_next_kid.clone();
    let token = jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(
            cfg.task_capability_next_secret
                .as_deref()
                .unwrap()
                .as_bytes(),
        ),
    )?;

    let resp = client
        .post(format!("{base}/v1/task/heartbeat"))
        .header(TASK_CAPABILITY_HEADER, token)
        .json(&serde_json::json!({
            "task_id": task_id,
            "attempt": attempt,
            "lease_token": lease_token,
        }))
        .send()
        .await?;

    anyhow::ensure!(
        resp.status().is_success(),
        "expected 200, got {}",
        resp.status()
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn stale_attempt_fencing_rejects_old_complete() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let mut cfg = migrated_config().await?;
    cfg.lease_duration_secs = 1;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let server = DispatcherServer::start(
        pool,
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        false,
        false,
        false,
    )
    .await?;

    let base = format!("http://{}", server.addr);
    let client = reqwest::Client::new();
    let task_id = Uuid::new_v4();

    let claim1 = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let attempt1 = claim1["attempt"].as_i64().context("attempt1")?;
    let lease1 = claim1["lease_token"].as_str().context("lease1")?;
    let token1 = claim1["capability_token"].as_str().context("token1")?;

    tokio::time::sleep(Duration::from_millis(1200)).await;

    let claim2 = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let attempt2 = claim2["attempt"].as_i64().context("attempt2")?;
    anyhow::ensure!(attempt2 == attempt1 + 1, "expected attempt bump");

    let resp = client
        .post(format!("{base}/v1/task/complete"))
        .header(TASK_CAPABILITY_HEADER, token1)
        .json(&serde_json::json!({
            "task_id": task_id,
            "attempt": attempt1,
            "lease_token": lease1,
            "outcome": "success",
        }))
        .send()
        .await?;

    anyhow::ensure!(
        resp.status() == reqwest::StatusCode::CONFLICT,
        "expected 409, got {}",
        resp.status()
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn stale_attempt_fencing_rejects_old_buffer_publish() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let mut cfg = migrated_config().await?;
    cfg.lease_duration_secs = 1;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let server = DispatcherServer::start(
        pool.clone(),
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        false,
        false,
        false,
    )
    .await?;

    let base = format!("http://{}", server.addr);
    let client = reqwest::Client::new();
    let task_id = Uuid::new_v4();

    let claim1 = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let attempt1 = claim1["attempt"].as_i64().context("attempt1")?;
    let lease1 = claim1["lease_token"].as_str().context("lease1")?;
    let token1 = claim1["capability_token"].as_str().context("token1")?;

    tokio::time::sleep(Duration::from_millis(1200)).await;

    let claim2 = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let attempt2 = claim2["attempt"].as_i64().context("attempt2")?;
    anyhow::ensure!(attempt2 == attempt1 + 1, "expected attempt bump");

    let resp = client
        .post(format!("{base}/v1/task/buffer-publish"))
        .header(TASK_CAPABILITY_HEADER, token1)
        .json(&serde_json::json!({
            "task_id": task_id,
            "attempt": attempt1,
            "lease_token": lease1,
            "batch_uri": format!("s3://{}/batches/{task_id}/{attempt1}.jsonl", cfg.s3_bucket),
            "content_type": CONTENT_TYPE_JSONL,
            "batch_size_bytes": 1,
            "dedupe_scope": "test",
        }))
        .send()
        .await?;

    anyhow::ensure!(
        resp.status() == reqwest::StatusCode::CONFLICT,
        "expected 409, got {}",
        resp.status()
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn buffer_publish_is_idempotent_for_same_attempt_and_uri() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let cfg = migrated_config().await?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let server = DispatcherServer::start(
        pool.clone(),
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        false,
        false,
        false,
    )
    .await?;

    let base = format!("http://{}", server.addr);
    let client = reqwest::Client::new();
    let task_id = Uuid::new_v4();

    let claim = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let attempt = claim["attempt"].as_i64().context("attempt")?;
    let lease = claim["lease_token"].as_str().context("lease_token")?;
    let token = claim["capability_token"]
        .as_str()
        .context("capability_token")?;

    let batch_uri = format!("s3://{}/batches/{task_id}/{attempt}.jsonl", cfg.s3_bucket);
    for _ in 0..2 {
        client
            .post(format!("{base}/v1/task/buffer-publish"))
            .header(TASK_CAPABILITY_HEADER, token)
            .json(&serde_json::json!({
                "task_id": task_id,
                "attempt": attempt,
                "lease_token": lease,
                "batch_uri": batch_uri.clone(),
                "content_type": CONTENT_TYPE_JSONL,
                "batch_size_bytes": 1,
                "dedupe_scope": "test",
            }))
            .send()
            .await?
            .error_for_status()?;
    }

    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM state.outbox
        WHERE topic = $1
          AND payload->>'batch_uri' = $2
        "#,
    )
    .bind(&cfg.buffer_queue)
    .bind(&batch_uri)
    .fetch_one(&pool)
    .await?;

    anyhow::ensure!(count == 1, "expected 1 outbox row, got {count}");

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn dispatcher_restart_recovers_outbox() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let cfg = migrated_config().await?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let server1 = DispatcherServer::start(
        pool.clone(),
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        false,
        false,
        false,
    )
    .await?;

    let base1 = format!("http://{}", server1.addr);
    let client = reqwest::Client::new();
    let task_id = Uuid::new_v4();

    let claim = client
        .post(format!("{base1}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let attempt = claim["attempt"].as_i64().context("attempt")?;
    let lease_token = claim["lease_token"].as_str().context("lease_token")?;
    let token = claim["capability_token"]
        .as_str()
        .context("capability_token")?;

    client
        .post(format!("{base1}/v1/task/buffer-publish"))
        .header(TASK_CAPABILITY_HEADER, token)
        .json(&serde_json::json!({
            "task_id": task_id,
            "attempt": attempt,
            "lease_token": lease_token,
            "batch_uri": format!("s3://{}/batches/{task_id}/{attempt}.jsonl", cfg.s3_bucket),
            "content_type": CONTENT_TYPE_JSONL,
            "batch_size_bytes": 1,
            "dedupe_scope": "test",
        }))
        .send()
        .await?
        .error_for_status()?;

    let pgq = PgQueue::new(pool.clone());
    let initial = pgq
        .receive(&cfg.buffer_queue, 10, Duration::from_secs(1))
        .await?;
    anyhow::ensure!(initial.is_empty(), "outbox should not be drained yet");

    server1.shutdown().await?;

    let server2 = DispatcherServer::start(
        pool.clone(),
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        true,
        false,
        false,
    )
    .await?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let got = pgq
            .receive(&cfg.buffer_queue, 10, Duration::from_secs(1))
            .await?;
        if let Some(msg) = got.first() {
            pgq.ack(&msg.ack_token).await?;
            break;
        }
        if tokio::time::Instant::now() > deadline {
            anyhow::bail!("timed out waiting for drained buffer message");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    server2.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn worker_crash_triggers_retry() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let mut cfg = migrated_config().await?;
    cfg.lease_duration_secs = 1;
    cfg.outbox_poll_ms = 50;
    cfg.lease_reaper_poll_ms = 50;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;
    let pgq = PgQueue::new(pool.clone());

    let server = DispatcherServer::start(
        pool.clone(),
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        true,
        false,
        true,
    )
    .await?;
    let base = format!("http://{}", server.addr);
    let client = reqwest::Client::new();

    let task_id = Uuid::new_v4();
    pgq.publish(
        &cfg.task_wakeup_queue,
        serde_json::json!({ "task_id": task_id }),
        chrono::Utc::now(),
    )
    .await?;

    let initial_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let wakeup = pgq
            .receive(&cfg.task_wakeup_queue, 1, Duration::from_secs(30))
            .await?;
        if wakeup.is_empty() {
            if tokio::time::Instant::now() > initial_deadline {
                anyhow::bail!("timed out waiting for initial wakeup");
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        }

        let msg = &wakeup[0];
        let got_task_id = msg
            .payload
            .get("task_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Uuid>().ok());

        pgq.ack(&msg.ack_token).await?;

        if got_task_id == Some(task_id) {
            break;
        }
    }

    let claim1 = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let attempt1 = claim1["attempt"].as_i64().context("attempt1")?;
    anyhow::ensure!(attempt1 == 1, "expected attempt 1");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let got = pgq
            .receive(&cfg.task_wakeup_queue, 1, Duration::from_secs(30))
            .await?;
        if got.is_empty() {
            if tokio::time::Instant::now() > deadline {
                let task_row: Option<(String, i64, Option<chrono::DateTime<chrono::Utc>>)> =
                    sqlx::query_as(
                        r#"
                        SELECT status, attempt, lease_expires_at
                        FROM state.tasks
                        WHERE task_id = $1
                        "#,
                    )
                    .bind(task_id)
                    .fetch_optional(&pool)
                    .await
                    .context("debug: fetch task row")?;

                let outbox_pending: i64 = sqlx::query_scalar(
                    r#"
                    SELECT count(*)
                    FROM state.outbox
                    WHERE topic = $1
                      AND status = 'pending'
                      AND payload->>'task_id' = $2
                    "#,
                )
                .bind(&cfg.task_wakeup_queue)
                .bind(task_id.to_string())
                .fetch_one(&pool)
                .await
                .context("debug: count pending outbox rows")?;

                let outbox_sent: i64 = sqlx::query_scalar(
                    r#"
                    SELECT count(*)
                    FROM state.outbox
                    WHERE topic = $1
                      AND status = 'sent'
                      AND payload->>'task_id' = $2
                    "#,
                )
                .bind(&cfg.task_wakeup_queue)
                .bind(task_id.to_string())
                .fetch_one(&pool)
                .await
                .context("debug: count sent outbox rows")?;

                let queue_visible: i64 = sqlx::query_scalar(
                    r#"
                    SELECT count(*)
                    FROM state.queue_messages
                    WHERE queue_name = $1
                      AND available_at <= now()
                      AND (invisible_until IS NULL OR invisible_until <= now())
                      AND payload->>'task_id' = $2
                    "#,
                )
                .bind(&cfg.task_wakeup_queue)
                .bind(task_id.to_string())
                .fetch_one(&pool)
                .await
                .context("debug: count visible queue messages")?;

                let queue_total: i64 = sqlx::query_scalar(
                    r#"
                    SELECT count(*)
                    FROM state.queue_messages
                    WHERE queue_name = $1
                      AND payload->>'task_id' = $2
                    "#,
                )
                .bind(&cfg.task_wakeup_queue)
                .bind(task_id.to_string())
                .fetch_one(&pool)
                .await
                .context("debug: count total queue messages")?;

                anyhow::bail!(
                    "timed out waiting for retry wakeup: task_row={task_row:?} outbox_pending={outbox_pending} outbox_sent={outbox_sent} queue_visible={queue_visible} queue_total={queue_total}"
                );
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }

        let msg = &got[0];
        let got_task_id = msg
            .payload
            .get("task_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Uuid>().ok());

        pgq.ack(&msg.ack_token).await?;

        if got_task_id == Some(task_id) {
            break;
        }
    }

    let claim2 = client
        .post(format!("{base}/internal/task-claim"))
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let attempt2 = claim2["attempt"].as_i64().context("attempt2")?;
    anyhow::ensure!(attempt2 == 2, "expected attempt 2, got {attempt2}");

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn poison_batch_goes_to_dlq_without_partial_writes() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let mut cfg = migrated_config().await?;
    cfg.sink_max_deliveries = 2;
    cfg.sink_retry_delay_ms = 100;

    let state_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;
    let data_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.data_database_url)
        .await
        .context("connect data db")?;
    let pgq = PgQueue::new(state_pool);
    let object_store = ObjectStore::new(&cfg.s3_endpoint)?;

    let dedupe_key = format!("poison_test:{}", Uuid::new_v4());
    let valid = serde_json::json!({
        "alert_definition_id": DEFAULT_ALERT_DEFINITION_ID,
        "dedupe_key": dedupe_key,
        "event_time": chrono::Utc::now().to_rfc3339(),
        "chain_id": 1,
        "block_number": 123,
        "block_hash": "0xblockhash",
        "tx_hash": "0xtxhash",
        "payload": {"ok": true},
    });
    let mut bytes = serde_json::to_vec(&valid)?;
    bytes.push(b'\n');
    bytes.extend_from_slice(b"not json\n");

    let key = format!("poison/{}.jsonl", Uuid::new_v4());
    object_store
        .put_bytes(&cfg.s3_bucket, &key, bytes.clone(), CONTENT_TYPE_JSONL)
        .await?;

    let batch_uri = format!("s3://{}/{}", cfg.s3_bucket, key);
    pgq.publish(
        &cfg.buffer_queue,
        serde_json::json!({
            "batch_uri": batch_uri,
            "content_type": CONTENT_TYPE_JSONL,
            "batch_size_bytes": bytes.len(),
        }),
        chrono::Utc::now(),
    )
    .await?;

    let sink_cfg = cfg.clone();
    let sink_task = tokio::spawn(async move { trace_harness::sink::run(&sink_cfg).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let got = pgq
            .receive(&cfg.buffer_queue_dlq, 1, Duration::from_secs(30))
            .await?;
        if !got.is_empty() {
            pgq.ack(&got[0].ack_token).await?;
            break;
        }
        if tokio::time::Instant::now() > deadline {
            sink_task.abort();
            anyhow::bail!("timed out waiting for DLQ message");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    sink_task.abort();

    let row = sqlx::query(
        r#"
        SELECT 1 FROM data.alert_events WHERE dedupe_key = $1
        "#,
    )
    .bind(&dedupe_key)
    .fetch_optional(&data_pool)
    .await?;
    anyhow::ensure!(row.is_none(), "expected no partial insert for poison batch");

    Ok(())
}

#[tokio::test]
async fn runner_claim_invoke_sink_inserts_once() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    #[derive(Debug, Deserialize)]
    struct ClaimResponse {
        task_id: Uuid,
        attempt: i64,
        lease_token: Uuid,
        lease_expires_at: chrono::DateTime<chrono::Utc>,
        capability_token: String,
        work_payload: serde_json::Value,
    }

    let mut cfg = migrated_config().await?;
    cfg.outbox_poll_ms = 50;
    cfg.sink_poll_ms = 50;

    let state_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;
    let data_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.data_database_url)
        .await
        .context("connect data db")?;

    let server = DispatcherServer::start(
        state_pool.clone(),
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        true,
        false,
        false,
    )
    .await?;
    let base = format!("http://{}", server.addr);

    let sink_cfg = cfg.clone();
    let sink_task = tokio::spawn(async move { trace_harness::sink::run(&sink_cfg).await });

    let result: anyhow::Result<()> = async {
        let object_store = ObjectStore::new(&cfg.s3_endpoint)?;
        let object_store_arc: Arc<dyn ObjectStoreTrait> = Arc::new(object_store.clone());
        let runner = FakeRunner::new(
            base.clone(),
            cfg.query_service_url.clone(),
            cfg.s3_bucket.clone(),
            object_store_arc,
        );
        let client = reqwest::Client::new();

        let task_id = Uuid::new_v4();
        let dedupe_key = format!("runner_test:{task_id}");
        let bundle = serde_json::json!({
            "alert_definition_id": DEFAULT_ALERT_DEFINITION_ID,
            "dedupe_key": dedupe_key,
            "chain_id": 1,
            "block_number": 123,
            "block_hash": "0xblockhash",
            "tx_hash": "0xtxhash",
            "payload": {"ok": true},
        });

        let bundle_key = format!("bundles/{task_id}.json");
        object_store
            .put_bytes(
                &cfg.s3_bucket,
                &bundle_key,
                serde_json::to_vec(&bundle).context("encode bundle")?,
                CONTENT_TYPE_JSON,
            )
            .await
            .context("upload bundle")?;

        let bundle_url = format!(
            "{}/{}/{}",
            cfg.s3_endpoint.trim_end_matches('/'),
            cfg.s3_bucket,
            bundle_key
        );

        sqlx::query(
            r#"
            INSERT INTO state.tasks (task_id, status, payload)
            VALUES ($1, 'queued', $2)
            ON CONFLICT (task_id) DO UPDATE
            SET status = EXCLUDED.status,
                payload = EXCLUDED.payload,
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = now()
            "#,
        )
        .bind(task_id)
        .bind(serde_json::json!({ "bundle_url": bundle_url }))
        .execute(&state_pool)
        .await
        .context("insert task")?;

        let claim: ClaimResponse = client
            .post(format!("{base}/internal/task-claim"))
            .json(&serde_json::json!({ "task_id": task_id }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let bundle_url = claim.work_payload["bundle_url"]
            .as_str()
            .context("bundle_url")?
            .to_string();

        let invocation = UdfInvocationPayload {
            task_id: claim.task_id,
            attempt: claim.attempt,
            lease_token: claim.lease_token,
            lease_expires_at: claim.lease_expires_at,
            capability_token: claim.capability_token,
            bundle_url,
            work_payload: claim.work_payload,
        };

        runner
            .invoke(&invocation)
            .await
            .map_err(anyhow::Error::from)?;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let count: i64 = sqlx::query_scalar(
                r#"
                SELECT count(*)
                FROM data.alert_events
                WHERE dedupe_key = $1
                "#,
            )
            .bind(&dedupe_key)
            .fetch_one(&data_pool)
            .await?;

            if count == 1 {
                break;
            }

            if tokio::time::Instant::now() > deadline {
                anyhow::bail!("timed out waiting for sink insert");
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        runner
            .invoke(&invocation)
            .await
            .map_err(anyhow::Error::from)?;

        let expected_batch_uri = format!(
            "s3://{}/batches/{}/{}/udf.jsonl",
            cfg.s3_bucket, invocation.task_id, invocation.attempt
        );

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)
            FROM state.outbox
            WHERE topic = $1
              AND payload->>'batch_uri' = $2
            "#,
        )
        .bind(&cfg.buffer_queue)
        .bind(&expected_batch_uri)
        .fetch_one(&state_pool)
        .await?;

        anyhow::ensure!(
            outbox_count == 1,
            "expected 1 outbox row for batch_uri, got {outbox_count}"
        );

        Ok(())
    }
    .await;

    sink_task.abort();
    server.shutdown().await?;
    result
}

#[tokio::test]
async fn dispatcher_dataset_grant_allows_task_query_and_emits_audit() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    #[derive(Debug, Deserialize)]
    struct ClaimResponse {
        attempt: i64,
        capability_token: String,
    }

    let cfg = migrated_config().await?;
    seed_alerts_fixture_dataset(&cfg).await?;

    let state_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let data_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.data_database_url)
        .await
        .context("connect data db")?;

    let server = DispatcherServer::start(
        state_pool,
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        false,
        false,
        false,
    )
    .await?;

    let mut qs_cfg = QueryServiceConfig::from_env().context("load query service config")?;
    qs_cfg.data_database_url = cfg.data_database_url.clone();
    qs_cfg.task_capability_iss = cfg.task_capability_iss.clone();
    qs_cfg.task_capability_aud = cfg.task_capability_aud.clone();
    qs_cfg.task_capability_kid = cfg.task_capability_kid.clone();
    qs_cfg.task_capability_secret = cfg.task_capability_secret.clone();
    qs_cfg.task_capability_next_kid = cfg.task_capability_next_kid.clone();
    qs_cfg.task_capability_next_secret = cfg.task_capability_next_secret.clone();
    qs_cfg.task_capability_ttl_secs = cfg.task_capability_ttl_secs;
    qs_cfg.s3_endpoint = cfg.s3_endpoint.clone();

    let qs_state = build_query_state(qs_cfg)
        .await
        .context("build query service state")?;
    let qs_app = query_router(qs_state);

    let qs_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("bind query service")?;
    let qs_addr = qs_listener
        .local_addr()
        .context("query service local_addr")?;
    let qs_base = format!("http://{qs_addr}");

    let (qs_shutdown_tx, qs_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let qs_task = tokio::spawn(async move {
        axum::serve(qs_listener, qs_app.into_make_service())
            .with_graceful_shutdown(async move {
                let _ = qs_shutdown_rx.await;
            })
            .await
            .context("query service serve")
    });

    let result = async {
        let dispatcher_base = format!("http://{}", server.addr);
        let client = reqwest::Client::new();
        let task_id = Uuid::new_v4();

        let claim = client
            .post(format!("{dispatcher_base}/internal/task-claim"))
            .json(&serde_json::json!({ "task_id": task_id }))
            .send()
            .await?
            .error_for_status()?
            .json::<ClaimResponse>()
            .await?;

        let req = TaskQueryRequest {
            task_id,
            attempt: claim.attempt,
            dataset_id: ALERTS_FIXTURE_DATASET_ID,
            sql: "SELECT dedupe_key FROM dataset ORDER BY dedupe_key".to_string(),
            limit: None,
        };

        let resp = client
            .post(format!("{qs_base}/v1/task/query"))
            .header(TASK_CAPABILITY_HEADER, &claim.capability_token)
            .json(&req)
            .send()
            .await?;

        anyhow::ensure!(
            resp.status() == reqwest::StatusCode::OK,
            "expected 200, got {}",
            resp.status()
        );

        let body = resp.json::<serde_json::Value>().await?;
        let rows = body["rows"]
            .as_array()
            .context("rows is array")?
            .iter()
            .map(|r| r[0].as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();

        anyhow::ensure!(
            rows == vec!["dedupe-001", "dedupe-002", "dedupe-003"],
            "unexpected rows: {rows:?}"
        );

        let unauthorized_dataset_id = Uuid::from_bytes([
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x99,
        ]);

        let unauthorized_req = TaskQueryRequest {
            task_id,
            attempt: claim.attempt,
            dataset_id: unauthorized_dataset_id,
            sql: "SELECT 1".to_string(),
            limit: None,
        };

        let unauthorized_resp = client
            .post(format!("{qs_base}/v1/task/query"))
            .header(TASK_CAPABILITY_HEADER, &claim.capability_token)
            .json(&unauthorized_req)
            .send()
            .await?;

        anyhow::ensure!(
            unauthorized_resp.status() == reqwest::StatusCode::FORBIDDEN,
            "expected 403, got {}",
            unauthorized_resp.status()
        );

        let audit_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)
            FROM data.query_audit
            WHERE task_id = $1
              AND dataset_id = $2
            "#,
        )
        .bind(task_id)
        .bind(ALERTS_FIXTURE_DATASET_ID)
        .fetch_one(&data_pool)
        .await?;

        anyhow::ensure!(audit_count == 1, "expected 1 audit row, got {audit_count}");

        let org_id: Uuid = sqlx::query_scalar(
            r#"
            SELECT org_id
            FROM data.query_audit
            WHERE task_id = $1
              AND dataset_id = $2
            ORDER BY query_time DESC
            LIMIT 1
            "#,
        )
        .bind(task_id)
        .bind(ALERTS_FIXTURE_DATASET_ID)
        .fetch_one(&data_pool)
        .await?;

        anyhow::ensure!(
            org_id == cfg.org_id,
            "expected org_id {}, got {org_id}",
            cfg.org_id
        );

        let result_row_count: i64 = sqlx::query_scalar(
            r#"
            SELECT result_row_count
            FROM data.query_audit
            WHERE task_id = $1
              AND dataset_id = $2
            ORDER BY query_time DESC
            LIMIT 1
            "#,
        )
        .bind(task_id)
        .bind(ALERTS_FIXTURE_DATASET_ID)
        .fetch_one(&data_pool)
        .await?;

        anyhow::ensure!(
            result_row_count == 3,
            "expected result_row_count 3, got {result_row_count}"
        );

        let unauthorized_audit_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)
            FROM data.query_audit
            WHERE task_id = $1
              AND dataset_id = $2
            "#,
        )
        .bind(task_id)
        .bind(unauthorized_dataset_id)
        .fetch_one(&data_pool)
        .await?;

        anyhow::ensure!(
            unauthorized_audit_count == 0,
            "expected no audit rows for unauthorized dataset, got {unauthorized_audit_count}"
        );

        Ok(())
    }
    .await;

    let _ = qs_shutdown_tx.send(());
    let _ = qs_task.await;
    let _ = server.shutdown().await;
    result
}

#[tokio::test]
async fn alert_evaluate_over_parquet_dataset_emits_idempotent_events_and_rejects_malformed(
) -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    #[derive(Debug, Deserialize)]
    struct ClaimResponse {
        task_id: Uuid,
        attempt: i64,
        lease_token: Uuid,
        lease_expires_at: chrono::DateTime<chrono::Utc>,
        capability_token: String,
        work_payload: serde_json::Value,
    }

    let mut cfg = migrated_config().await?;
    cfg.outbox_poll_ms = 50;
    cfg.sink_poll_ms = 50;

    let state_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;
    let data_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.data_database_url)
        .await
        .context("connect data db")?;

    let dataset_payload = CryoIngestPayload {
        dataset_uuid: Uuid::new_v4(),
        chain_id: 1,
        range_start: 0,
        range_end: 3,
        config_hash: "cryo_ingest.blocks:v1".to_string(),
        dataset_key: None,
        cryo_dataset_name: None,
        rpc_pool: None,
    };
    let dataset_pubd = derive_dataset_publication(&cfg.s3_bucket, &dataset_payload);

    let capability: Arc<dyn SignerTrait> = Arc::new(TaskCapability::from_hs256_config(
        Hs256TaskCapabilityConfig {
            issuer: cfg.task_capability_iss.clone(),
            audience: cfg.task_capability_aud.clone(),
            current_kid: cfg.task_capability_kid.clone(),
            current_secret: cfg.task_capability_secret.clone(),
            next_kid: cfg.task_capability_next_kid.clone(),
            next_secret: cfg.task_capability_next_secret.clone(),
            ttl: Duration::from_secs(cfg.task_capability_ttl_secs),
        },
    )?);

    let queue: Arc<dyn trace_core::Queue> = Arc::new(PgQueue::new(state_pool.clone()));
    let dataset_storage_prefix = match &dataset_pubd.storage_ref {
        trace_core::DatasetStorageRef::S3 { bucket, prefix, .. } => {
            format!("s3://{bucket}/{prefix}")
        }
        trace_core::DatasetStorageRef::File { .. } => {
            anyhow::bail!("expected s3 dataset storage ref")
        }
    };
    let dispatcher_cfg = trace_dispatcher::DispatcherConfig {
        org_id: cfg.org_id,
        lease_duration_secs: cfg.lease_duration_secs,
        outbox_poll_ms: cfg.outbox_poll_ms,
        lease_reaper_poll_ms: cfg.lease_reaper_poll_ms,
        outbox_batch_size: cfg.outbox_batch_size,
        task_wakeup_queue: cfg.task_wakeup_queue.clone(),
        buffer_queue: cfg.buffer_queue.clone(),
        default_datasets: vec![DatasetGrant {
            dataset_uuid: dataset_pubd.dataset_uuid,
            dataset_version: dataset_pubd.dataset_version,
            storage_ref: Some(dataset_pubd.storage_ref.clone()),
        }],
        default_s3: S3Grants {
            read_prefixes: vec![dataset_storage_prefix],
            write_prefixes: Vec::new(),
        },
    };

    let dispatcher = trace_dispatcher::DispatcherServer::start(
        state_pool.clone(),
        dispatcher_cfg,
        capability,
        queue,
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        true,
        false,
        false,
    )
    .await?;
    let dispatcher_base = format!("http://{}", dispatcher.addr);

    let mut qs_cfg = QueryServiceConfig::from_env().context("load query service config")?;
    qs_cfg.data_database_url = cfg.data_database_url.clone();
    qs_cfg.task_capability_iss = cfg.task_capability_iss.clone();
    qs_cfg.task_capability_aud = cfg.task_capability_aud.clone();
    qs_cfg.task_capability_kid = cfg.task_capability_kid.clone();
    qs_cfg.task_capability_secret = cfg.task_capability_secret.clone();
    qs_cfg.task_capability_next_kid = cfg.task_capability_next_kid.clone();
    qs_cfg.task_capability_next_secret = cfg.task_capability_next_secret.clone();
    qs_cfg.task_capability_ttl_secs = cfg.task_capability_ttl_secs;
    qs_cfg.s3_endpoint = cfg.s3_endpoint.clone();

    let qs_state = build_query_state(qs_cfg)
        .await
        .context("build query service state")?;
    let qs_app = query_router(qs_state);
    let qs_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("bind query service")?;
    let qs_addr = qs_listener
        .local_addr()
        .context("query service local_addr")?;
    let qs_base = format!("http://{qs_addr}");

    let (qs_shutdown_tx, qs_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let qs_task = tokio::spawn(async move {
        axum::serve(qs_listener, qs_app.into_make_service())
            .with_graceful_shutdown(async move {
                let _ = qs_shutdown_rx.await;
            })
            .await
            .context("query service serve")
    });

    let sink_cfg = cfg.clone();
    let sink_task = tokio::spawn(async move { trace_harness::sink::run(&sink_cfg).await });

    let result: anyhow::Result<()> = async {
        let object_store = ObjectStore::new(&cfg.s3_endpoint)?;
        let object_store_arc: Arc<dyn ObjectStoreTrait> = Arc::new(object_store.clone());
        let runner = FakeRunner::new(
            dispatcher_base.clone(),
            qs_base.clone(),
            cfg.s3_bucket.clone(),
            object_store_arc,
        );
        let dispatcher_client = DispatcherClient::new(dispatcher_base.clone());

        // Produce a real Parquet dataset version via the trusted Cryo worker path.
        let cryo_task_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO state.tasks (task_id, status, payload)
            VALUES ($1, 'queued', $2)
            ON CONFLICT (task_id) DO UPDATE
            SET status = EXCLUDED.status,
                payload = EXCLUDED.payload,
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = now()
            "#,
        )
        .bind(cryo_task_id)
        .bind(serde_json::to_value(&dataset_payload).context("encode cryo payload")?)
        .execute(&state_pool)
        .await
        .context("insert cryo task")?;

        let res = trace_harness::cryo_worker::run_task(
            &cfg,
            &object_store as &dyn ObjectStoreTrait,
            &dispatcher_client,
            cryo_task_id,
        )
        .await?;
        anyhow::ensure!(res.is_some(), "expected cryo task to complete");

        // Alert evaluation task: query the attached dataset and emit one alert event per row.
        let eval_task_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO state.tasks (task_id, status, payload)
            VALUES ($1, 'queued', $2)
            ON CONFLICT (task_id) DO UPDATE
            SET status = EXCLUDED.status,
                payload = EXCLUDED.payload,
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = now()
            "#,
        )
        .bind(eval_task_id)
        .bind(serde_json::json!({
            "bundle_url": format!("s3://{}/bundles/{}.json", cfg.s3_bucket, eval_task_id),
            "alert_evaluate": {
                "alert_definition_id": DEFAULT_ALERT_DEFINITION_ID,
                "dataset_id": dataset_pubd.dataset_uuid,
                "dataset_version": dataset_pubd.dataset_version,
                "sql": "SELECT chain_id, block_number, block_hash FROM dataset ORDER BY block_number",
                "retry_once": true
            }
        }))
        .execute(&state_pool)
        .await
        .context("insert eval task")?;

        let claim1: ClaimResponse = reqwest::Client::new()
            .post(format!("{dispatcher_base}/internal/task-claim"))
            .json(&serde_json::json!({ "task_id": eval_task_id }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let invocation1 = UdfInvocationPayload {
            task_id: claim1.task_id,
            attempt: claim1.attempt,
            lease_token: claim1.lease_token,
            lease_expires_at: claim1.lease_expires_at,
            capability_token: claim1.capability_token,
            bundle_url: claim1.work_payload["bundle_url"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            work_payload: claim1.work_payload,
        };
        runner
            .invoke(&invocation1)
            .await
            .map_err(anyhow::Error::from)?;

        let claim2: ClaimResponse = reqwest::Client::new()
            .post(format!("{dispatcher_base}/internal/task-claim"))
            .json(&serde_json::json!({ "task_id": eval_task_id }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let invocation2 = UdfInvocationPayload {
            task_id: claim2.task_id,
            attempt: claim2.attempt,
            lease_token: claim2.lease_token,
            lease_expires_at: claim2.lease_expires_at,
            capability_token: claim2.capability_token,
            bundle_url: claim2.work_payload["bundle_url"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            work_payload: claim2.work_payload,
        };
        runner
            .invoke(&invocation2)
            .await
            .map_err(anyhow::Error::from)?;

        let expected_keys = [0_i64, 1_i64, 2_i64]
            .into_iter()
            .map(|block_number| {
                format!(
                    "alert_eval:{}:{}:{}",
                    DEFAULT_ALERT_DEFINITION_ID, dataset_pubd.dataset_version, block_number
                )
            })
            .collect::<Vec<_>>();

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let count: i64 = sqlx::query_scalar(
                r#"
                SELECT count(*)
                FROM data.alert_events
                WHERE dedupe_key = ANY($1)
                "#,
            )
            .bind(&expected_keys)
            .fetch_one(&data_pool)
            .await?;

            if count == expected_keys.len() as i64 {
                break;
            }

            if tokio::time::Instant::now() > deadline {
                anyhow::bail!("timed out waiting for alert_events insert");
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let audit_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)
            FROM data.query_audit
            WHERE task_id = $1
              AND dataset_id = $2
            "#,
        )
        .bind(eval_task_id)
        .bind(dataset_pubd.dataset_uuid)
        .fetch_one(&data_pool)
        .await?;
        anyhow::ensure!(audit_count >= 1, "expected at least 1 audit row");

        // Malformed UDF output is rejected before buffer publish; no outbox/sink writes occur.
        let bad_task_id = Uuid::new_v4();
        let bad_alert_definition_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO state.tasks (task_id, status, payload)
            VALUES ($1, 'queued', $2)
            ON CONFLICT (task_id) DO UPDATE
            SET status = EXCLUDED.status,
                payload = EXCLUDED.payload,
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = now()
            "#,
        )
        .bind(bad_task_id)
        .bind(serde_json::json!({
            "bundle_url": format!("s3://{}/bundles/{}.json", cfg.s3_bucket, bad_task_id),
            "alert_evaluate": {
                "alert_definition_id": bad_alert_definition_id,
                "dataset_id": dataset_pubd.dataset_uuid,
                "dataset_version": dataset_pubd.dataset_version,
                "sql": "SELECT chain_id, block_number, block_hash FROM dataset ORDER BY block_number",
                "emit_malformed_output": true
            }
        }))
        .execute(&state_pool)
        .await
        .context("insert malformed eval task")?;

        let bad_claim: ClaimResponse = reqwest::Client::new()
            .post(format!("{dispatcher_base}/internal/task-claim"))
            .json(&serde_json::json!({ "task_id": bad_task_id }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let bad_invocation = UdfInvocationPayload {
            task_id: bad_claim.task_id,
            attempt: bad_claim.attempt,
            lease_token: bad_claim.lease_token,
            lease_expires_at: bad_claim.lease_expires_at,
            capability_token: bad_claim.capability_token,
            bundle_url: bad_claim.work_payload["bundle_url"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            work_payload: bad_claim.work_payload,
        };
        runner
            .invoke(&bad_invocation)
            .await
            .map_err(anyhow::Error::from)?;

        let malformed_dedupe_key = format!(
            "alert_eval:{}:{}:malformed",
            bad_alert_definition_id, dataset_pubd.dataset_version
        );
        let bad_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)
            FROM data.alert_events
            WHERE dedupe_key = $1
            "#,
        )
        .bind(&malformed_dedupe_key)
        .fetch_one(&data_pool)
        .await?;
        anyhow::ensure!(
            bad_count == 0,
            "expected no alert_events rows for malformed output"
        );

        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)
            FROM state.outbox
            WHERE topic = $1
              AND payload->>'task_id' = $2
            "#,
        )
        .bind(&cfg.buffer_queue)
        .bind(bad_task_id.to_string())
        .fetch_one(&state_pool)
        .await?;
        anyhow::ensure!(
            outbox_count == 0,
            "expected no outbox buffer publish rows for malformed task"
        );

        Ok(())
    }
    .await;

    sink_task.abort();
    let _ = qs_shutdown_tx.send(());
    let _ = qs_task.await;
    let _ = dispatcher.shutdown().await;
    result
}

#[tokio::test]
async fn cryo_worker_registers_dataset_version_idempotently() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let cfg = migrated_config().await?;
    let state_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let server = DispatcherServer::start(
        state_pool.clone(),
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        false,
        false,
        false,
    )
    .await?;
    let base = format!("http://{}", server.addr);

    let result: anyhow::Result<()> = async {
        let object_store = ObjectStore::new(&cfg.s3_endpoint)?;
        let dispatcher = DispatcherClient::new(base);

        let payload = CryoIngestPayload {
            dataset_uuid: Uuid::new_v4(),
            chain_id: 1,
            range_start: 100,
            range_end: 102,
            config_hash: "cfg_hash_v1".to_string(),
            dataset_key: None,
            cryo_dataset_name: None,
            rpc_pool: None,
        };
        let expected = derive_dataset_publication(&cfg.s3_bucket, &payload);

        for task_id in [Uuid::new_v4(), Uuid::new_v4()] {
            sqlx::query(
                r#"
                INSERT INTO state.tasks (task_id, status, payload)
                VALUES ($1, 'queued', $2)
                ON CONFLICT (task_id) DO UPDATE
                SET status = EXCLUDED.status,
                    payload = EXCLUDED.payload,
                    lease_token = NULL,
                    lease_expires_at = NULL,
                    updated_at = now()
                "#,
            )
            .bind(task_id)
            .bind(serde_json::to_value(&payload).context("encode payload")?)
            .execute(&state_pool)
            .await
            .context("insert cryo task")?;

            let res = trace_harness::cryo_worker::run_task(
                &cfg,
                &object_store as &dyn ObjectStoreTrait,
                &dispatcher,
                task_id,
            )
            .await?;
            anyhow::ensure!(res.is_some(), "expected cryo task to complete");
        }

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)
            FROM state.dataset_versions
            WHERE dataset_uuid = $1
              AND config_hash = $2
              AND range_start = $3
              AND range_end = $4
            "#,
        )
        .bind(expected.dataset_uuid)
        .bind(&expected.config_hash)
        .bind(expected.range_start)
        .bind(expected.range_end)
        .fetch_one(&state_pool)
        .await
        .context("count dataset_versions")?;

        anyhow::ensure!(count == 1, "expected 1 dataset_versions row, got {count}");

        let (dataset_version, storage_prefix, storage_glob): (Uuid, String, String) =
            sqlx::query_as(
                r#"
            SELECT dataset_version, storage_prefix, storage_glob
            FROM state.dataset_versions
            WHERE dataset_uuid = $1
              AND config_hash = $2
              AND range_start = $3
              AND range_end = $4
            LIMIT 1
            "#,
            )
            .bind(expected.dataset_uuid)
            .bind(&expected.config_hash)
            .bind(expected.range_start)
            .bind(expected.range_end)
            .fetch_one(&state_pool)
            .await
            .context("fetch dataset_versions row")?;

        anyhow::ensure!(
            dataset_version == expected.dataset_version,
            "expected dataset_version {}, got {dataset_version}",
            expected.dataset_version
        );
        let expected_storage_prefix = match &expected.storage_ref {
            trace_core::DatasetStorageRef::S3 { bucket, prefix, .. } => {
                format!("s3://{bucket}/{prefix}")
            }
            trace_core::DatasetStorageRef::File { prefix, .. } => format!("file://{prefix}"),
        };
        anyhow::ensure!(
            storage_prefix == expected_storage_prefix,
            "expected storage_prefix {}, got {storage_prefix}",
            expected_storage_prefix
        );
        let expected_storage_glob = match &expected.storage_ref {
            trace_core::DatasetStorageRef::S3 { glob, .. } => glob.clone(),
            trace_core::DatasetStorageRef::File { glob, .. } => glob.clone(),
        };
        anyhow::ensure!(
            storage_glob == expected_storage_glob,
            "expected storage_glob {}, got {storage_glob}",
            expected_storage_glob
        );

        let (bucket, prefix_key) = parse_s3_uri(&storage_prefix).context("parse storage prefix")?;

        let parquet_key = join_key(
            &prefix_key,
            &format!("cryo_{}_{}.parquet", payload.range_start, payload.range_end),
        );
        let parquet_bytes = object_store
            .get_bytes(&bucket, &parquet_key)
            .await
            .context("read parquet")?;
        anyhow::ensure!(
            !parquet_bytes.is_empty(),
            "expected parquet object to be non-empty"
        );

        Ok(())
    }
    .await;

    server.shutdown().await?;
    result
}

#[tokio::test]
async fn query_service_attaches_local_parquet_via_file_storage_ref() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let cfg = migrated_config().await?;

    let signer = TaskCapability::from_hs256_config(Hs256TaskCapabilityConfig {
        issuer: cfg.task_capability_iss.clone(),
        audience: cfg.task_capability_aud.clone(),
        current_kid: cfg.task_capability_kid.clone(),
        current_secret: cfg.task_capability_secret.clone(),
        next_kid: cfg.task_capability_next_kid.clone(),
        next_secret: cfg.task_capability_next_secret.clone(),
        ttl: Duration::from_secs(cfg.task_capability_ttl_secs),
    })?;

    let root = std::env::temp_dir().join(format!("trace-qs-local-{}", Uuid::new_v4()));
    let dataset_dir = root.join("datasets").join("alerts_fixture");
    std::fs::create_dir_all(&dataset_dir).context("create dataset dir")?;

    let parquet_path = dataset_dir.join("alerts_fixture.parquet");
    tokio::task::spawn_blocking({
        let parquet_path = parquet_path.clone();
        move || -> anyhow::Result<()> {
            let conn = Connection::open_in_memory().context("open duckdb in-memory")?;
            conn.execute_batch(
                r#"
                BEGIN;
                CREATE TABLE alerts_fixture (
                  dedupe_key VARCHAR NOT NULL
                );
                INSERT INTO alerts_fixture VALUES
                  ('dedupe-001'),
                  ('dedupe-002'),
                  ('dedupe-003');
                COMMIT;
                "#,
            )
            .context("create fixture table")?;

            let parquet_escaped = parquet_path.to_string_lossy().replace('\'', "''");
            conn.execute_batch(&format!(
                "COPY alerts_fixture TO '{parquet_escaped}' (FORMAT PARQUET);"
            ))
            .context("copy to parquet")?;
            Ok(())
        }
    })
    .await
    .context("join parquet builder")??;

    let task_id = Uuid::new_v4();
    let dataset_id = Uuid::new_v4();
    let dataset_version = Uuid::new_v4();
    let token = signer.issue_task_capability(&TaskCapabilityIssueRequest {
        org_id: cfg.org_id,
        task_id,
        attempt: 1,
        datasets: vec![DatasetGrant {
            dataset_uuid: dataset_id,
            dataset_version,
            storage_ref: Some(trace_core::DatasetStorageRef::File {
                prefix: format!("{}/", dataset_dir.to_string_lossy()),
                glob: "*.parquet".to_string(),
            }),
        }],
        s3: S3Grants::empty(),
    })?;

    let mut qs_cfg = QueryServiceConfig::from_env().context("load query service config")?;
    qs_cfg.data_database_url = cfg.data_database_url.clone();
    qs_cfg.task_capability_iss = cfg.task_capability_iss.clone();
    qs_cfg.task_capability_aud = cfg.task_capability_aud.clone();
    qs_cfg.task_capability_kid = cfg.task_capability_kid.clone();
    qs_cfg.task_capability_secret = cfg.task_capability_secret.clone();
    qs_cfg.task_capability_next_kid = cfg.task_capability_next_kid.clone();
    qs_cfg.task_capability_next_secret = cfg.task_capability_next_secret.clone();
    qs_cfg.task_capability_ttl_secs = cfg.task_capability_ttl_secs;
    qs_cfg.allow_local_files = true;
    qs_cfg.local_file_root = Some(root.to_string_lossy().to_string());

    let qs_state = build_query_state(qs_cfg)
        .await
        .context("build query service state")?;
    let qs_app = query_router(qs_state);
    let qs_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("bind query service")?;
    let qs_addr = qs_listener
        .local_addr()
        .context("query service local_addr")?;
    let qs_base = format!("http://{qs_addr}");

    let (qs_shutdown_tx, qs_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let qs_task = tokio::spawn(async move {
        axum::serve(qs_listener, qs_app.into_make_service())
            .with_graceful_shutdown(async move {
                let _ = qs_shutdown_rx.await;
            })
            .await
            .context("query service serve")
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{qs_base}/v1/task/query"))
        .header(TASK_CAPABILITY_HEADER, token)
        .json(&TaskQueryRequest {
            task_id,
            attempt: 1,
            dataset_id,
            sql: "SELECT dedupe_key FROM dataset ORDER BY dedupe_key".to_string(),
            limit: None,
        })
        .send()
        .await?
        .error_for_status()?;

    let body: serde_json::Value = resp.json().await?;
    let rows = body["rows"]
        .as_array()
        .context("rows is array")?
        .iter()
        .map(|r| r[0].as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert_eq!(rows, vec!["dedupe-001", "dedupe-002", "dedupe-003"]);

    let _ = qs_shutdown_tx.send(());
    let _ = qs_task.await;
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[tokio::test]
async fn planner_bootstrap_sync_schedules_and_completes_ranges() -> anyhow::Result<()> {
    let _lock = integration_lock().await;
    let cfg = migrated_config().await?;

    let state_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let data_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.data_database_url)
        .await
        .context("connect data db")?;

    let server = DispatcherServer::start(
        state_pool.clone(),
        cfg.clone(),
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        true,
        true,
        false,
    )
    .await?;
    let base = format!("http://{}", server.addr);

    let result: anyhow::Result<()> = async {
        let chain_id = ((Uuid::new_v4().as_u128() % 1_000_000) as i64) + 1;
        let chunk_size: i64 = 1000;
        let to_block: i64 = 3 * chunk_size;
        let name = format!("chain_sync_test_{}", Uuid::new_v4());
        let yaml = format!(
            r#"
kind: chain_sync
name: {name}
chain_id: {chain_id}
mode:
  kind: fixed_target
  from_block: 0
  to_block: {to_block}
streams:
  blocks:
    cryo_dataset_name: blocks
    rpc_pool: standard
    chunk_size: {chunk_size}
    max_inflight: 10
  geth_calls:
    cryo_dataset_name: geth_calls
    rpc_pool: traces
    chunk_size: {chunk_size}
    max_inflight: 10
"#
        );

        let applied = apply_chain_sync_yaml(&state_pool, cfg.org_id, &yaml)
            .await
            .context("apply chain_sync yaml")?;

        // Regression guard: rpc_pool flows YAML -> state.chain_sync_streams.
        for (dataset_key, expected_pool) in [("blocks", "standard"), ("geth_calls", "traces")] {
            let stored: String = sqlx::query_scalar(
                r#"
                SELECT rpc_pool
                FROM state.chain_sync_streams
                WHERE job_id = $1
                  AND dataset_key = $2
                "#,
            )
            .bind(applied.job_id)
            .bind(dataset_key)
            .fetch_one(&state_pool)
            .await
            .with_context(|| format!("fetch rpc_pool for {dataset_key}"))?;

            anyhow::ensure!(
                stored == expected_pool,
                "expected rpc_pool {expected_pool} for {dataset_key}, got {stored}"
            );
        }

        let object_store = ObjectStore::new(&cfg.s3_endpoint)?;
        let dispatcher = DispatcherClient::new(base.clone());
        let queue = PgQueue::new(state_pool.clone());

        let visibility_timeout = Duration::from_secs(cfg.worker_visibility_timeout_secs);
        let expected_completed: i64 = 2 * 3;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);

        loop {
            let msgs = queue
                .receive(&cfg.task_wakeup_queue, 10, visibility_timeout)
                .await?;

            for msg in &msgs {
                let task_id = msg.payload["task_id"]
                    .as_str()
                    .context("task_id")?
                    .parse::<Uuid>()
                    .context("parse task_id")?;

                // Regression guard: rpc_pool flows state.chain_sync_streams -> state.tasks.payload.
                let payload: serde_json::Value = sqlx::query_scalar(
                    r#"
                    SELECT payload
                    FROM state.tasks
                    WHERE task_id = $1
                    "#,
                )
                .bind(task_id)
                .fetch_one(&state_pool)
                .await
                .context("fetch task payload")?;

                let dataset_key = payload["dataset_key"]
                    .as_str()
                    .context("payload.dataset_key")?;
                let rpc_pool = payload["rpc_pool"].as_str().context("payload.rpc_pool")?;
                match dataset_key {
                    "blocks" => anyhow::ensure!(
                        rpc_pool == "standard",
                        "expected rpc_pool standard for blocks task, got {rpc_pool}"
                    ),
                    "geth_calls" => anyhow::ensure!(
                        rpc_pool == "traces",
                        "expected rpc_pool traces for geth_calls task, got {rpc_pool}"
                    ),
                    other => {
                        anyhow::bail!("unexpected dataset_key in planned task payload: {other}")
                    }
                }

                let _res = trace_harness::cryo_worker::run_task(
                    &cfg,
                    &object_store as &dyn ObjectStoreTrait,
                    &dispatcher,
                    task_id,
                )
                .await?;
            }

            for msg in msgs {
                queue.ack(&msg.ack_token).await?;
            }

            let completed: i64 = sqlx::query_scalar(
                r#"
                SELECT count(*)
                FROM state.chain_sync_scheduled_ranges
                WHERE job_id = $1
                  AND status = 'completed'
                "#,
            )
            .bind(applied.job_id)
            .fetch_one(&state_pool)
            .await?;

            if completed == expected_completed {
                break;
            }

            if tokio::time::Instant::now() > deadline {
                anyhow::bail!("timed out waiting for cryo ranges to complete");
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let blocks_uuid = derive_dataset_uuid(cfg.org_id, chain_id, "blocks")?;
        let calls_uuid = derive_dataset_uuid(cfg.org_id, chain_id, "geth_calls")?;

        for (dataset_uuid, config_hash) in [
            (blocks_uuid, "cryo_ingest.blocks:v1"),
            (calls_uuid, "cryo_ingest.geth_calls:v1"),
        ] {
            let dataset_rows: i64 = sqlx::query_scalar(
                r#"
                SELECT count(*)
                FROM state.dataset_versions
                WHERE dataset_uuid = $1
                  AND config_hash = $2
                  AND range_start IN (0, 1000, 2000)
                  AND range_end IN (1000, 2000, 3000)
                "#,
            )
            .bind(dataset_uuid)
            .bind(config_hash)
            .fetch_one(&state_pool)
            .await
            .context("count dataset_versions")?;

            anyhow::ensure!(
                dataset_rows == 3,
                "expected 3 dataset versions for {config_hash}, got {dataset_rows}"
            );
        }

        let (dataset_version, storage_prefix, storage_glob): (Uuid, String, String) =
            sqlx::query_as(
                r#"
            SELECT dataset_version, storage_prefix, storage_glob
            FROM state.dataset_versions
            WHERE dataset_uuid = $1
              AND config_hash = 'cryo_ingest.blocks:v1'
            ORDER BY range_start
            LIMIT 1
            "#,
            )
            .bind(blocks_uuid)
            .fetch_one(&state_pool)
            .await?;

        // Prove Query Service can attach+query one produced dataset version.
        let signer = TaskCapability::from_hs256_config(Hs256TaskCapabilityConfig {
            issuer: cfg.task_capability_iss.clone(),
            audience: cfg.task_capability_aud.clone(),
            current_kid: cfg.task_capability_kid.clone(),
            current_secret: cfg.task_capability_secret.clone(),
            next_kid: cfg.task_capability_next_kid.clone(),
            next_secret: cfg.task_capability_next_secret.clone(),
            ttl: Duration::from_secs(cfg.task_capability_ttl_secs),
        })?;

        let query_task_id = Uuid::new_v4();
        let (grant_bucket, grant_prefix) =
            trace_core::lite::s3::parse_s3_uri(&storage_prefix).context("parse storage prefix")?;
        let issue_req = TaskCapabilityIssueRequest {
            org_id: cfg.org_id,
            task_id: query_task_id,
            attempt: 1,
            datasets: vec![DatasetGrant {
                dataset_uuid: blocks_uuid,
                dataset_version,
                storage_ref: Some(trace_core::DatasetStorageRef::S3 {
                    bucket: grant_bucket,
                    prefix: grant_prefix,
                    glob: storage_glob.clone(),
                }),
            }],
            s3: S3Grants {
                read_prefixes: vec![storage_prefix.clone()],
                write_prefixes: Vec::new(),
            },
        };
        let token = signer.issue_task_capability(&issue_req)?;

        let mut qs_cfg = QueryServiceConfig::from_env().context("load query service config")?;
        qs_cfg.data_database_url = cfg.data_database_url.clone();
        qs_cfg.task_capability_iss = cfg.task_capability_iss.clone();
        qs_cfg.task_capability_aud = cfg.task_capability_aud.clone();
        qs_cfg.task_capability_kid = cfg.task_capability_kid.clone();
        qs_cfg.task_capability_secret = cfg.task_capability_secret.clone();
        qs_cfg.task_capability_next_kid = cfg.task_capability_next_kid.clone();
        qs_cfg.task_capability_next_secret = cfg.task_capability_next_secret.clone();
        qs_cfg.task_capability_ttl_secs = cfg.task_capability_ttl_secs;
        qs_cfg.s3_endpoint = cfg.s3_endpoint.clone();

        let qs_state = build_query_state(qs_cfg)
            .await
            .context("build query service state")?;
        let qs_app = query_router(qs_state);
        let qs_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let qs_addr = qs_listener.local_addr()?;
        let qs_base = format!("http://{qs_addr}");

        let (qs_shutdown_tx, qs_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let qs_task = tokio::spawn(async move {
            axum::serve(qs_listener, qs_app.into_make_service())
                .with_graceful_shutdown(async move {
                    let _ = qs_shutdown_rx.await;
                })
                .await
                .context("query service serve")
        });

        let query_result: anyhow::Result<()> = async {
            let client = reqwest::Client::new();
            let req = TaskQueryRequest {
                task_id: query_task_id,
                attempt: 1,
                dataset_id: blocks_uuid,
                sql: "SELECT count(*) FROM dataset".to_string(),
                limit: None,
            };

            let resp = client
                .post(format!("{qs_base}/v1/task/query"))
                .header(TASK_CAPABILITY_HEADER, token)
                .json(&req)
                .send()
                .await?;

            let status = resp.status();
            let body = resp.json::<serde_json::Value>().await?;
            anyhow::ensure!(status.is_success(), "expected 200, got {status}: {body}");
            let count = body["rows"][0][0].as_i64().context("count")?;
            anyhow::ensure!(count > 0, "expected non-empty dataset, got {count}");

            let audit: i64 = sqlx::query_scalar(
                r#"
                SELECT count(*)
                FROM data.query_audit
                WHERE task_id = $1
                  AND dataset_id = $2
                "#,
            )
            .bind(query_task_id)
            .bind(blocks_uuid)
            .fetch_one(&data_pool)
            .await?;

            anyhow::ensure!(audit == 1, "expected 1 audit row, got {audit}");
            Ok(())
        }
        .await;

        let _ = qs_shutdown_tx.send(());
        let _ = qs_task.await;
        query_result?;

        let cursor_next: i64 = sqlx::query_scalar(
            r#"
            SELECT next_block
            FROM state.chain_sync_cursor
            WHERE job_id = $1
              AND dataset_key = $2
            "#,
        )
        .bind(applied.job_id)
        .bind("blocks")
        .fetch_one(&state_pool)
        .await?;

        let calls_cursor_next: i64 = sqlx::query_scalar(
            r#"
            SELECT next_block
            FROM state.chain_sync_cursor
            WHERE job_id = $1
              AND dataset_key = $2
            "#,
        )
        .bind(applied.job_id)
        .bind("geth_calls")
        .fetch_one(&state_pool)
        .await?;

        anyhow::ensure!(
            cursor_next == to_block,
            "expected blocks cursor {to_block}, got {cursor_next}"
        );
        anyhow::ensure!(
            calls_cursor_next == to_block,
            "expected geth_calls cursor {to_block}, got {calls_cursor_next}"
        );

        Ok(())
    }
    .await;

    server.shutdown().await?;
    result
}
