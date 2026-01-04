use anyhow::Context;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use tower::util::ServiceExt;
use trace_core::fixtures::{ALERTS_FIXTURE_DATASET_ID, ALERTS_FIXTURE_DATASET_VERSION};
use trace_core::lite::jwt::{Hs256TaskCapabilityConfig, TaskCapability};
use trace_core::Signer as _;
use trace_core::{DatasetGrant, S3Grants, TaskCapabilityIssueRequest};
use trace_query_service::{
    build_state, config::QueryServiceConfig, router, TaskQueryRequest, TASK_CAPABILITY_HEADER,
};
use uuid::Uuid;

fn issue_token(
    cfg: &QueryServiceConfig,
    task_id: Uuid,
    attempt: i64,
    dataset_uuids: &[Uuid],
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
        datasets: dataset_uuids
            .iter()
            .copied()
            .map(|dataset_uuid| DatasetGrant {
                dataset_uuid,
                dataset_version: ALERTS_FIXTURE_DATASET_VERSION,
            })
            .collect(),
        s3: S3Grants::empty(),
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
    assert_eq!(status, StatusCode::OK);
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
        sql: "SELECT dedupe_key FROM alerts_fixture ORDER BY dedupe_key".to_string(),
        limit: None,
    };
    let (status, body) = send_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::OK);
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
    assert_eq!(status, StatusCode::OK);

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
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04,
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
