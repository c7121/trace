use anyhow::Context;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use trace_core::{udf::UdfInvocationPayload, ObjectStore as ObjectStoreTrait};
use trace_harness::{
    config::HarnessConfig, dispatcher::DispatcherServer, migrate, pgqueue::PgQueue, s3::ObjectStore,
    runner::FakeRunner,
};
use uuid::Uuid;

fn unique_queue(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4())
}

async fn migrated_config() -> anyhow::Result<HarnessConfig> {
    let mut cfg = HarnessConfig::from_env().context("load harness config")?;
    cfg.task_wakeup_queue = unique_queue("task_wakeup_test");
    cfg.buffer_queue = unique_queue("buffer_queue_test");
    cfg.buffer_queue_dlq = unique_queue("buffer_queue_dlq_test");
    migrate::run(&cfg).await.context("run migrations")?;
    Ok(cfg)
}

#[tokio::test]
async fn duplicate_claims_do_not_double_run() -> anyhow::Result<()> {
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
        .header("X-Trace-Task-Capability", token_a)
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
        .header("X-Trace-Task-Capability", token)
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
        .header("X-Trace-Task-Capability", token)
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
        .header("X-Trace-Task-Capability", token1)
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
        .header("X-Trace-Task-Capability", token1)
        .json(&serde_json::json!({
            "task_id": task_id,
            "attempt": attempt1,
            "lease_token": lease1,
            "batch_uri": format!("s3://{}/batches/{task_id}/{attempt1}.jsonl", cfg.s3_bucket),
            "content_type": "application/jsonl",
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
            .header("X-Trace-Task-Capability", token)
            .json(&serde_json::json!({
                "task_id": task_id,
                "attempt": attempt,
                "lease_token": lease,
                "batch_uri": batch_uri.clone(),
                "content_type": "application/jsonl",
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
        .header("X-Trace-Task-Capability", token)
        .json(&serde_json::json!({
            "task_id": task_id,
            "attempt": attempt,
            "lease_token": lease_token,
            "batch_uri": format!("s3://{}/batches/{task_id}/{attempt}.jsonl", cfg.s3_bucket),
            "content_type": "application/jsonl",
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
                anyhow::bail!("timed out waiting for retry wakeup");
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
        "alert_definition_id": "490b8f3f-1d41-496a-917b-5b7eeeb85e07",
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
        .put_bytes(&cfg.s3_bucket, &key, bytes.clone(), "application/jsonl")
        .await?;

    let batch_uri = format!("s3://{}/{}", cfg.s3_bucket, key);
    pgq.publish(
        &cfg.buffer_queue,
        serde_json::json!({
            "batch_uri": batch_uri,
            "content_type": "application/jsonl",
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
    )
    .await?;
    let base = format!("http://{}", server.addr);

    let sink_cfg = cfg.clone();
    let sink_task = tokio::spawn(async move { trace_harness::sink::run(&sink_cfg).await });

    let result: anyhow::Result<()> = async {
        let object_store = ObjectStore::new(&cfg.s3_endpoint)?;
        let object_store_arc: Arc<dyn ObjectStoreTrait> = Arc::new(object_store.clone());
        let runner = FakeRunner::new(base.clone(), cfg.s3_bucket.clone(), object_store_arc);
        let client = reqwest::Client::new();

        let task_id = Uuid::new_v4();
        let dedupe_key = format!("runner_test:{task_id}");
        let bundle = serde_json::json!({
            "alert_definition_id": "490b8f3f-1d41-496a-917b-5b7eeeb85e07",
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
                "application/json",
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

        runner.run(&invocation).await?;

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

        runner.run(&invocation).await?;

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
