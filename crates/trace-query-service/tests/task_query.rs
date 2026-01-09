use anyhow::Context;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use duckdb::Connection;
use http_body_util::BodyExt;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::util::ServiceExt;
use trace_core::fixtures::{
    ALERTS_FIXTURE_DATASET_ID, ALERTS_FIXTURE_DATASET_STORAGE_PREFIX,
    ALERTS_FIXTURE_DATASET_VERSION,
};
use trace_core::lite::jwt::{Hs256TaskCapabilityConfig, TaskCapability};
use trace_core::lite::s3::parse_s3_uri;
use trace_core::manifest::DatasetManifestV1;
use trace_core::Signer as _;
use trace_core::{DatasetGrant, DatasetStorageRef, S3Grants, TaskCapabilityIssueRequest};
use trace_query_service::{
    build_state, config::QueryServiceConfig, router, AppState, TaskQueryRequest, UserQueryRequest,
    TASK_CAPABILITY_HEADER,
};
use uuid::Uuid;

const CONTENT_TYPE_PARQUET: &str = "application/octet-stream";
const CONTENT_TYPE_JSON: &str = "application/json";

fn init_tracing() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt::try_init();
    });
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

            // Install once for test runs so Query Service can `LOAD httpfs` without network access
            // during request handling.
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

fn fixture_grants(dataset_uuids: &[Uuid]) -> anyhow::Result<(Vec<DatasetGrant>, S3Grants)> {
    let (bucket, prefix) = parse_s3_uri(ALERTS_FIXTURE_DATASET_STORAGE_PREFIX)
        .context("parse fixture storage prefix")?;

    let datasets = dataset_uuids
        .iter()
        .copied()
        .map(|dataset_uuid| DatasetGrant {
            dataset_uuid,
            dataset_version: ALERTS_FIXTURE_DATASET_VERSION,
            storage_ref: (dataset_uuid == ALERTS_FIXTURE_DATASET_ID).then(|| {
                DatasetStorageRef::S3 {
                    bucket: bucket.clone(),
                    prefix: prefix.clone(),
                    glob: "*.parquet".to_string(),
                }
            }),
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

    Ok((datasets, s3))
}

fn issue_token(
    cfg: &QueryServiceConfig,
    task_id: Uuid,
    attempt: i64,
    dataset_uuids: &[Uuid],
) -> anyhow::Result<String> {
    let (datasets, s3) = fixture_grants(dataset_uuids)?;

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

#[derive(Debug, Serialize)]
struct UserJwtClaims {
    iss: String,
    aud: String,
    sub: String,
    exp: usize,
    iat: usize,

    org_id: Uuid,
    datasets: Vec<DatasetGrant>,
    s3: S3Grants,
}

fn issue_user_token(
    cfg: &QueryServiceConfig,
    sub: &str,
    dataset_uuids: &[Uuid],
) -> anyhow::Result<String> {
    let (datasets, s3) = fixture_grants(dataset_uuids)?;
    issue_user_token_with_datasets(cfg, sub, datasets, s3, &cfg.user_jwt_secret)
}

fn issue_user_token_with_datasets(
    cfg: &QueryServiceConfig,
    sub: &str,
    datasets: Vec<DatasetGrant>,
    s3: S3Grants,
    secret: &str,
) -> anyhow::Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("read clock")?
        .as_secs();
    let iat: usize = now.try_into().unwrap_or(0);
    let exp: usize = (now + 300).try_into().unwrap_or(usize::MAX);

    let claims = UserJwtClaims {
        iss: cfg.user_jwt_iss.clone(),
        aud: cfg.user_jwt_aud.clone(),
        sub: sub.to_string(),
        exp,
        iat,
        org_id: Uuid::parse_str("00000000-0000-0000-0000-000000000001")?,
        datasets,
        s3,
    };

    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some(cfg.user_jwt_kid.clone());
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .context("encode user jwt")
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

async fn send_user_query(
    app: axum::Router,
    token: Option<String>,
    req: &UserQueryRequest,
) -> anyhow::Result<(StatusCode, serde_json::Value)> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/v1/query")
        .header("content-type", "application/json");

    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }

    let request = builder.body(Body::from(serde_json::to_vec(req)?))?;
    let response = app.oneshot(request).await?;
    let status = response.status();
    let bytes = response.into_body().collect().await?.to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes)?;
    Ok((status, body))
}

async fn setup() -> anyhow::Result<(QueryServiceConfig, sqlx::PgPool, axum::Router)> {
    init_tracing();
    ensure_duckdb_httpfs_installed()?;

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

#[derive(Clone)]
struct RecordingObjectStore {
    inner: trace_core::lite::s3::ObjectStore,
    gets: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl trace_core::ObjectStore for RecordingObjectStore {
    async fn put_bytes(
        &self,
        bucket: &str,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> trace_core::Result<()> {
        self.inner.put_bytes(bucket, key, bytes, content_type).await
    }

    async fn put_file(
        &self,
        bucket: &str,
        key: &str,
        local_path: &std::path::Path,
        content_type: &str,
    ) -> trace_core::Result<()> {
        self.inner
            .put_file(bucket, key, local_path, content_type)
            .await
    }

    async fn get_bytes(&self, bucket: &str, key: &str) -> trace_core::Result<Vec<u8>> {
        self.gets
            .lock()
            .expect("mutex poisoned")
            .push(format!("{bucket}/{key}"));

        if key.to_ascii_lowercase().ends_with(".parquet") {
            return Err(trace_core::Error::msg(
                "regression: query service must not download parquet bytes via object store",
            ));
        }
        self.inner.get_bytes(bucket, key).await
    }
}

async fn setup_with_recording_store() -> anyhow::Result<(
    QueryServiceConfig,
    sqlx::PgPool,
    axum::Router,
    Arc<Mutex<Vec<String>>>,
)> {
    init_tracing();
    ensure_duckdb_httpfs_installed()?;

    let cfg = QueryServiceConfig::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.data_database_url)
        .await?;

    sqlx::migrate!("../../harness/migrations/data")
        .run(&pool)
        .await?;

    let state = build_state(cfg.clone()).await?;
    let AppState {
        cfg: state_cfg,
        signer,
        user_jwt,
        duckdb,
        data_pool,
        object_store: _,
    } = state;

    let gets = Arc::new(Mutex::new(Vec::new()));
    let inner = trace_core::lite::s3::ObjectStore::new(&cfg.s3_endpoint)?;
    let object_store: Arc<dyn trace_core::ObjectStore> = Arc::new(RecordingObjectStore {
        inner,
        gets: gets.clone(),
    });

    let state = AppState {
        cfg: state_cfg,
        signer,
        user_jwt,
        duckdb,
        data_pool,
        object_store,
    };

    let app = router(state);
    seed_fixture_dataset(&cfg, ALERTS_FIXTURE_DATASET_STORAGE_PREFIX).await?;
    Ok((cfg, pool, app, gets))
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
async fn dataset_storage_ref_must_be_under_s3_grants() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let task_id = Uuid::new_v4();
    let attempt = 1;
    let dataset_id = ALERTS_FIXTURE_DATASET_ID;

    let (grant_bucket, grant_prefix) = parse_s3_uri(ALERTS_FIXTURE_DATASET_STORAGE_PREFIX)
        .context("parse fixture storage prefix")?;
    let unauthorized_prefix = format!("s3://{grant_bucket}/not-authorized/");
    let token = issue_token_with_datasets(
        &cfg,
        task_id,
        attempt,
        vec![DatasetGrant {
            dataset_uuid: dataset_id,
            dataset_version: ALERTS_FIXTURE_DATASET_VERSION,
            storage_ref: Some(DatasetStorageRef::S3 {
                bucket: grant_bucket,
                prefix: grant_prefix,
                glob: "*.parquet".to_string(),
            }),
        }],
        S3Grants {
            read_prefixes: vec![unauthorized_prefix],
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
async fn query_service_fetches_only_manifest_via_object_store() -> anyhow::Result<()> {
    let (cfg, _pool, app, gets) = setup_with_recording_store().await?;

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

    let (status, body) = send_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let got = gets.lock().expect("mutex poisoned").clone();
    assert!(
        got.iter().all(|p| p.ends_with("/_manifest.json")),
        "unexpected get_bytes targets: {got:?}"
    );
    assert!(!got.is_empty(), "expected manifest fetch");
    Ok(())
}

#[tokio::test]
async fn manifest_parquet_keys_must_be_under_dataset_prefix_directory() -> anyhow::Result<()> {
    init_tracing();
    ensure_duckdb_httpfs_installed()?;

    let (cfg, _pool, app) = setup().await?;
    let (bucket, _fixture_prefix) = parse_s3_uri(ALERTS_FIXTURE_DATASET_STORAGE_PREFIX)
        .context("parse fixture storage prefix")?;

    // Use a unique keyspace per test run to avoid collisions across parallel test execution.
    let keyspace = format!("prefix-collision/{}/", Uuid::new_v4());

    // The dataset's declared prefix is `.../p/`.
    let good_prefix = format!("{keyspace}p/");
    // But the capability grant intentionally omits the trailing slash (`.../p`).
    // Without *directory* normalization, that could accidentally grant access to `.../p-evil/...`.
    let good_prefix_no_trailing_slash = format!("{keyspace}p");

    // This key is *not* under `good_prefix` as a directory, but it does start with
    // `good_prefix_no_trailing_slash`.
    let evil_parquet_key = format!("{keyspace}p-evil/evil.parquet");

    let dataset_uuid = Uuid::new_v4();
    let dataset_version = Uuid::new_v4();

    // Seed a valid Parquet object at the evil key.
    let object_store = trace_core::lite::s3::ObjectStore::new(&cfg.s3_endpoint)?;
    let parquet_bytes = build_fixture_parquet_bytes().await?;
    object_store
        .put_bytes(
            &bucket,
            &evil_parquet_key,
            parquet_bytes,
            CONTENT_TYPE_PARQUET,
        )
        .await?;

    // Write a manifest *under the good prefix* that tries to reference the evil key.
    let manifest_key = format!("{}/_manifest.json", good_prefix.trim_end_matches('/'));
    let manifest = DatasetManifestV1 {
        version: DatasetManifestV1::VERSION,
        dataset_uuid,
        dataset_version,
        parquet_keys: vec![evil_parquet_key.clone()],
    };
    let manifest_bytes = serde_json::to_vec(&manifest)?;
    object_store
        .put_bytes(&bucket, &manifest_key, manifest_bytes, CONTENT_TYPE_JSON)
        .await?;

    // Issue a token that (a) grants the dataset, and (b) grants an S3 prefix *without* a trailing slash.
    let task_id = Uuid::new_v4();
    let attempt = 1;
    let token = issue_token_with_datasets(
        &cfg,
        task_id,
        attempt,
        vec![DatasetGrant {
            dataset_uuid,
            dataset_version,
            storage_ref: Some(DatasetStorageRef::S3 {
                bucket: bucket.clone(),
                prefix: good_prefix.clone(),
                glob: "*.parquet".to_string(),
            }),
        }],
        S3Grants {
            read_prefixes: vec![format!("s3://{}/{}", bucket, good_prefix_no_trailing_slash)],
            write_prefixes: vec![],
        },
    )?;

    let body = TaskQueryRequest {
        task_id,
        attempt,
        dataset_id: dataset_uuid,
        sql: "SELECT 1 AS one".to_string(),
        limit: Some(1),
    };

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/task/query")
                .header(TASK_CAPABILITY_HEADER, token)
                .header(header::CONTENT_TYPE, CONTENT_TYPE_JSON)
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    Ok(())
}

#[tokio::test]
async fn user_auth_required_missing_token() -> anyhow::Result<()> {
    let (_cfg, _pool, app) = setup().await?;

    let req = UserQueryRequest {
        dataset_id: ALERTS_FIXTURE_DATASET_ID,
        sql: "SELECT 1".to_string(),
        limit: None,
    };

    let (status, _body) = send_user_query(app, None, &req).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn user_invalid_token_rejected() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let (datasets, s3) = fixture_grants(&[dataset_id])?;
    let token = issue_user_token_with_datasets(&cfg, "user:test", datasets, s3, "wrong-secret")?;

    let req = UserQueryRequest {
        dataset_id,
        sql: "SELECT 1".to_string(),
        limit: None,
    };

    let (status, _body) = send_user_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn user_dataset_grant_required() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_user_token(&cfg, "user:test", &[])?;

    let req = UserQueryRequest {
        dataset_id,
        sql: "SELECT 1".to_string(),
        limit: None,
    };

    let (status, _body) = send_user_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn user_gate_rejects_unsafe_sql() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_user_token(&cfg, "user:test", &[dataset_id])?;

    for sql in [
        "INSTALL httpfs",
        "LOAD httpfs",
        "ATTACH 'foo.db' AS other",
        "SELECT * FROM read_csv('data')",
        "SELECT * FROM read_parquet('http://example.com/x.parquet')",
        "SELECT * FROM 'local.csv'",
    ] {
        let req = UserQueryRequest {
            dataset_id,
            sql: sql.to_string(),
            limit: None,
        };
        let (status, _body) = send_user_query(app.clone(), Some(token.clone()), &req).await?;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "sql should be rejected: {sql}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn user_overblocking_url_literal_allowed() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_user_token(&cfg, "user:test", &[dataset_id])?;

    let req = UserQueryRequest {
        dataset_id,
        sql: "SELECT 'https://example.com'".to_string(),
        limit: None,
    };
    let (status, body) = send_user_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["rows"][0][0].as_str(), Some("https://example.com"));

    Ok(())
}

#[tokio::test]
async fn user_allowed_select_returns_deterministic_fixture() -> anyhow::Result<()> {
    let (cfg, _pool, app) = setup().await?;

    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_user_token(&cfg, "user:test", &[dataset_id])?;

    let req = UserQueryRequest {
        dataset_id,
        sql: "SELECT dedupe_key FROM dataset ORDER BY dedupe_key".to_string(),
        limit: None,
    };
    let (status, body) = send_user_query(app, Some(token), &req).await?;
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
async fn user_audit_emitted_after_success() -> anyhow::Result<()> {
    let (cfg, pool, app) = setup().await?;

    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let user_sub = "user:test";
    let token = issue_user_token(&cfg, user_sub, &[dataset_id])?;

    let req = UserQueryRequest {
        dataset_id,
        sql: "SELECT 1".to_string(),
        limit: None,
    };
    let (status, body) = send_user_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let row = sqlx::query(
        r#"
        SELECT org_id, user_sub, dataset_id, result_row_count, columns_accessed
        FROM data.user_query_audit
        ORDER BY query_time DESC
        LIMIT 1
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let org_id: Uuid = row.try_get("org_id")?;
    let logged_user_sub: String = row.try_get("user_sub")?;
    let logged_dataset_id: Uuid = row.try_get("dataset_id")?;
    let result_row_count: i64 = row.try_get("result_row_count")?;
    let columns_accessed: Option<serde_json::Value> = row.try_get("columns_accessed")?;

    assert_eq!(
        org_id,
        Uuid::parse_str("00000000-0000-0000-0000-000000000001")?
    );
    assert_eq!(logged_user_sub, user_sub);
    assert_eq!(logged_dataset_id, dataset_id);
    assert_eq!(result_row_count, 1);
    assert!(columns_accessed.is_none());

    Ok(())
}

#[tokio::test]
async fn user_query_fetches_only_manifest_via_object_store() -> anyhow::Result<()> {
    let (cfg, _pool, app, gets) = setup_with_recording_store().await?;

    let dataset_id = ALERTS_FIXTURE_DATASET_ID;
    let token = issue_user_token(&cfg, "user:test", &[dataset_id])?;

    let req = UserQueryRequest {
        dataset_id,
        sql: "SELECT 1".to_string(),
        limit: None,
    };

    let (status, body) = send_user_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let got = gets.lock().expect("mutex poisoned").clone();
    assert!(
        got.iter().all(|p| p.ends_with("/_manifest.json")),
        "unexpected get_bytes targets: {got:?}"
    );
    assert!(!got.is_empty(), "expected manifest fetch");
    Ok(())
}
