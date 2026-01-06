use anyhow::Context;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use duckdb::Connection;
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use tower::util::ServiceExt;
use trace_core::fixtures::{
    ALERTS_FIXTURE_DATASET_ID, ALERTS_FIXTURE_DATASET_STORAGE_PREFIX,
    ALERTS_FIXTURE_DATASET_VERSION,
};
use trace_core::lite::jwt::{Hs256TaskCapabilityConfig, TaskCapability};
use trace_core::lite::s3::parse_s3_uri;
use trace_core::Signer as _;
use trace_core::{DatasetGrant, S3Grants, TaskCapabilityIssueRequest};
use trace_query_service::{
    build_state, config::QueryServiceConfig, router, TaskQueryRequest, TASK_CAPABILITY_HEADER,
};
use uuid::Uuid;

const CONTENT_TYPE_JSON: &str = "application/json";
const CONTENT_TYPE_PARQUET: &str = "application/octet-stream";

#[derive(Clone)]
struct RecordingObjectStore {
    inner: std::sync::Arc<dyn trace_core::ObjectStore>,
    gets: std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
}

impl RecordingObjectStore {
    fn wrap(
        inner: std::sync::Arc<dyn trace_core::ObjectStore>,
    ) -> (Self, std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>) {
        let gets = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        (
            Self {
                inner,
                gets: gets.clone(),
            },
            gets,
        )
    }
}

impl trace_core::ObjectStore for RecordingObjectStore {
    fn put_bytes<'a>(
        &'a self,
        bucket: &'a str,
        key: &'a str,
        bytes: Vec<u8>,
        content_type: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = trace_core::Result<()>> + Send + 'a>,
    > {
        let inner = self.inner.clone();
        let bucket = bucket.to_string();
        let key = key.to_string();
        let content_type = content_type.to_string();
        Box::pin(async move {
            inner
                .put_bytes(&bucket, &key, bytes, &content_type)
                .await
        })
    }

    fn get_bytes<'a>(
        &'a self,
        bucket: &'a str,
        key: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = trace_core::Result<Vec<u8>>> + Send + 'a>,
    > {
        let inner = self.inner.clone();
        let gets = self.gets.clone();
        let bucket = bucket.to_string();
        let key = key.to_string();
        Box::pin(async move {
            {
                let mut log = gets.lock().expect("lock gets log");
                log.push((bucket.clone(), key.clone()));
            }
            inner.get_bytes(&bucket, &key).await
        })
    }
}

fn join_key(prefix: &str, leaf: &str) -> String {
    let prefix = prefix.trim_end_matches('/');
    format!("{prefix}/{leaf}")
}

async fn build_fixture_parquet_bytes() -> anyhow::Result<Vec<u8>> {
    tokio::task::spawn_blocking(|| {
        let dir = std::env::temp_dir().join(format!("trace-query-fixture-{}", Uuid::new_v4()));
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

async fn seed_fixture_dataset(
    cfg: &QueryServiceConfig,
    storage_prefix: &str,
) -> anyhow::Result<()> {
    let object_store = trace_core::lite::s3::ObjectStore::new(&cfg.s3_endpoint)?;
    let (bucket, prefix_key) = parse_s3_uri(storage_prefix).context("parse storage prefix")?;

    let parquet_bytes = build_fixture_parquet_bytes().await?;
    let parquet_key = join_key(&prefix_key, "alerts_fixture.parquet");
    object_store
        .put_bytes(&bucket, &parquet_key, parquet_bytes, CONTENT_TYPE_PARQUET)
        .await?;

    let parquet_uri = format!("s3://{bucket}/{parquet_key}");
    let manifest = serde_json::json!({
        "parquet_objects": [parquet_uri],
    });
    let manifest_key = join_key(&prefix_key, "_manifest.json");
    object_store
        .put_bytes(
            &bucket,
            &manifest_key,
            serde_json::to_vec(&manifest)?,
            CONTENT_TYPE_JSON,
        )
        .await?;

    Ok(())
}

fn issue_token(
    cfg: &QueryServiceConfig,
    task_id: Uuid,
    attempt: i64,
    dataset_uuids: &[Uuid],
) -> anyhow::Result<String> {
    let datasets = dataset_uuids
        .iter()
        .copied()
        .map(|dataset_uuid| DatasetGrant {
            dataset_uuid,
            dataset_version: ALERTS_FIXTURE_DATASET_VERSION,
            storage_prefix: (dataset_uuid == ALERTS_FIXTURE_DATASET_ID)
                .then(|| ALERTS_FIXTURE_DATASET_STORAGE_PREFIX.to_string()),
        })
        .collect::<Vec<_>>();

    let s3 = if dataset_uuids
        .iter()
        .any(|id| *id == ALERTS_FIXTURE_DATASET_ID)
    {
        S3Grants {
            read_prefixes: vec![ALERTS_FIXTURE_DATASET_STORAGE_PREFIX.to_string()],
            write_prefixes: Vec::new(),
        }
    } else {
        S3Grants::empty()
    };

    issue_token_with_datasets(cfg, task_id, attempt, datasets, s3)
}

fn issue_token_with_datasets(
    cfg: &QueryServiceConfig,
    task_id: Uuid,
    attempt: i64,
    datasets: Vec<DatasetGrant>,
    s3: S3Grants,
) -> anyhow::Result<String> {
    let signer = TaskCapability::from_hs256_config(Hs256TaskCapabilityConfig {
        issuer: cfg.task_capability_iss.clone(),
        audience: cfg.task_capability_aud.clone(),
        current_kid: cfg.task_capability_kid.clone(),
        current_secret: cfg.task_capability_secret.clone(),
        next_kid: cfg.task_capability_next_kid.clone(),
        next_secret: cfg.task_capability_next_secret.clone(),
        ttl: std::time::Duration::from_secs(cfg.task_capability_ttl_secs),
    })?;

    let req = TaskCapabilityIssueRequest {
        org_id: Uuid::parse_str("00000000-0000-0000-0000-000000000001")?,
        task_id,
        attempt,
        datasets,
        s3,
    };
    Ok(signer.issue_task_capability(&req)?)
}

async fn send_query(
    app: axum::Router,
    token: Option<String>,
    req: &TaskQueryRequest,
) -> anyhow::Result<(StatusCode, serde_json::Value)> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/v1/task/query")
        .header("content-type", "application/json");

    if let Some(token) = token {
        builder = builder.header(TASK_CAPABILITY_HEADER, token);
    }

    let request = builder.body(Body::from(serde_json::to_vec(req)?))?;
    let response = app.oneshot(request).await?;
    let status = response.status();
    let bytes = response.into_body().collect().await?.to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes)?;
    Ok((status, body))
}

async fn setup() -> anyhow::Result<(QueryServiceConfig, sqlx::PgPool, axum::Router)> {
    let cfg = QueryServiceConfig::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.data_database_url)
        .await?;

    sqlx::migrate!("../../harness/migrations/data")
        .run(&pool)
        .await?;

    let state = build_state(cfg.clone()).await?;
    let app = router(state);
    seed_fixture_dataset(&cfg, ALERTS_FIXTURE_DATASET_STORAGE_PREFIX).await?;
    Ok((cfg, pool, app))
}

#[tokio::test]
async fn auth_required_missing_token() -> anyhow::Result<()> {
    let (_cfg, _pool, app) = setup().await?;

    let req = TaskQueryRequest {
        task_id: Uuid::new_v4(),
        attempt: 1,
        dataset_id: ALERTS_FIXTURE_DATASET_ID,
        sql: "SELECT 1".to_string(),
        limit: None,
    };

    let (status, _body) = send_query(app, None, &req).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn wrong_token_rejected() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let task_id = Uuid::new_v4();
    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_token(&cfg, Uuid::new_v4(), 1, &[dataset_id])?;

    let req = TaskQueryRequest {
        task_id,
        attempt: 1,
        dataset_id,
        sql: "SELECT 1".to_string(),
        limit: None,
    };

    let (status, _body) = send_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn gate_rejects_unsafe_sql() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let task_id = Uuid::new_v4();
    let attempt = 1;
    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_token(&cfg, task_id, attempt, &[dataset_id])?;

    for sql in [
        "INSTALL httpfs",
        "LOAD httpfs",
        "ATTACH 'foo.db' AS other",
        "SELECT * FROM read_csv('data')",
        "SELECT * FROM read_parquet('http://example.com/x.parquet')",
        "SELECT * FROM 'local.csv'",
    ] {
        let req = TaskQueryRequest {
            task_id,
            attempt,
            dataset_id,
            sql: sql.to_string(),
            limit: None,
        };
        let (status, _body) = send_query(app.clone(), Some(token.clone()), &req).await?;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "sql should be rejected: {sql}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn overblocking_url_literal_allowed() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let task_id = Uuid::new_v4();
    let attempt = 1;
    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_token(&cfg, task_id, attempt, &[dataset_id])?;

    // Allow URL strings as inert literals (not external reads).
    let req = TaskQueryRequest {
        task_id,
        attempt,
        dataset_id,
        sql: "SELECT 'https://example.com'".to_string(),
        limit: None,
    };
    let (status, body) = send_query(app.clone(), Some(token.clone()), &req).await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["rows"][0][0].as_str(), Some("https://example.com"));

    Ok(())
}

#[tokio::test]
async fn allowed_select_returns_deterministic_fixture() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let task_id = Uuid::new_v4();
    let attempt = 1;
    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_token(&cfg, task_id, attempt, &[dataset_id])?;

    let req = TaskQueryRequest {
        task_id,
        attempt,
        dataset_id,
        sql: "SELECT dedupe_key FROM dataset ORDER BY dedupe_key".to_string(),
        limit: None,
    };
    let (status, body) = send_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["truncated"].as_bool(), Some(false));

    let rows = body["rows"]
        .as_array()
        .context("rows is array")?
        .iter()
        .map(|r| r[0].as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert_eq!(rows, vec!["dedupe-001", "dedupe-002", "dedupe-003"]);
    Ok(())
}

#[tokio::test]
async fn audit_emitted_after_success() -> anyhow::Result<()> {
    let (cfg, pool, app) = setup().await?;

    let task_id = Uuid::new_v4();
    let attempt = 1;
    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_token(&cfg, task_id, attempt, &[dataset_id])?;

    let req = TaskQueryRequest {
        task_id,
        attempt,
        dataset_id,
        sql: "SELECT 1".to_string(),
        limit: None,
    };
    let (status, _body) = send_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::OK, "body: {_body}");

    let row = sqlx::query(
        r#"
        SELECT org_id, dataset_id, result_row_count, columns_accessed
        FROM data.query_audit
        WHERE task_id = $1
        ORDER BY query_time DESC
        LIMIT 1
        "#,
    )
    .bind(task_id)
    .fetch_one(&pool)
    .await?;

    let org_id: Uuid = row.try_get("org_id")?;
    let logged_dataset_id: Uuid = row.try_get("dataset_id")?;
    let result_row_count: i64 = row.try_get("result_row_count")?;
    let columns_accessed: Option<serde_json::Value> = row.try_get("columns_accessed")?;

    assert_eq!(
        org_id,
        Uuid::parse_str("00000000-0000-0000-0000-000000000001")?
    );
    assert_eq!(logged_dataset_id, dataset_id);
    assert_eq!(result_row_count, 1);
    assert!(columns_accessed.is_none());

    Ok(())
}

#[tokio::test]
async fn dataset_grant_required() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let task_id = Uuid::new_v4();
    let attempt = 1;
    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_token(&cfg, task_id, attempt, &[])?;

    let req = TaskQueryRequest {
        task_id,
        attempt,
        dataset_id,
        sql: "SELECT 1".to_string(),
        limit: None,
    };

    let (status, _body) = send_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn dataset_grant_must_match_request() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let task_id = Uuid::new_v4();
    let attempt = 1;
    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let other_dataset_id = Uuid::from_bytes([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x04,
    ]);
    let token = issue_token(&cfg, task_id, attempt, &[other_dataset_id])?;

    let req = TaskQueryRequest {
        task_id,
        attempt,
        dataset_id,
        sql: "SELECT 1".to_string(),
        limit: None,
    };

    let (status, _body) = send_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn dataset_manifest_cannot_escape_s3_grants() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let task_id = Uuid::new_v4();
    let attempt = 1;
    let dataset_id = ALERTS_FIXTURE_DATASET_ID;

    let (bucket, _) = parse_s3_uri(ALERTS_FIXTURE_DATASET_STORAGE_PREFIX)?;
    let storage_prefix = format!("s3://{bucket}/tests/query/escape-{}/", Uuid::new_v4());

    let object_store = trace_core::lite::s3::ObjectStore::new(&cfg.s3_endpoint)?;
    let (bucket, prefix_key) = parse_s3_uri(&storage_prefix)?;
    let manifest_key = join_key(&prefix_key, "_manifest.json");
    let manifest = serde_json::json!({
        "parquet_objects": [format!("s3://{bucket}/outside/not-authorized.parquet")],
    });
    object_store
        .put_bytes(
            &bucket,
            &manifest_key,
            serde_json::to_vec(&manifest)?,
            CONTENT_TYPE_JSON,
        )
        .await?;

    let token = issue_token_with_datasets(
        &cfg,
        task_id,
        attempt,
        vec![DatasetGrant {
            dataset_uuid: dataset_id,
            dataset_version: ALERTS_FIXTURE_DATASET_VERSION,
            storage_prefix: Some(storage_prefix.clone()),
        }],
        S3Grants {
            read_prefixes: vec![storage_prefix],
            write_prefixes: Vec::new(),
        },
    )?;

    let req = TaskQueryRequest {
        task_id,
        attempt,
        dataset_id,
        sql: "SELECT 1".to_string(),
        limit: None,
    };

    let (status, _body) = send_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn query_service_fetches_manifest_only() -> anyhow::Result<()> {
    let cfg = QueryServiceConfig::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.data_database_url)
        .await?;

    sqlx::migrate!("../../harness/migrations/data")
        .run(&pool)
        .await?;

    let mut state = build_state(cfg.clone()).await?;
    let (store, gets) = RecordingObjectStore::wrap(state.object_store.clone());
    state.object_store = std::sync::Arc::new(store);

    let app = router(state);
    seed_fixture_dataset(&cfg, ALERTS_FIXTURE_DATASET_STORAGE_PREFIX).await?;

    let task_id = Uuid::new_v4();
    let attempt = 1;
    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_token(&cfg, task_id, attempt, &[dataset_id])?;

    let req = TaskQueryRequest {
        task_id,
        attempt,
        dataset_id,
        sql: "SELECT count(*) FROM dataset".to_string(),
        limit: None,
    };

    let (status, body) = send_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let reads = gets.lock().expect("lock gets log").clone();
    assert!(
        !reads.is_empty(),
        "expected manifest fetch via object store"
    );
    assert!(
        reads.iter().all(|(_bucket, key)| key.ends_with("_manifest.json")),
        "expected only manifest get_bytes calls, got: {reads:?}"
    );

    Ok(())
}
