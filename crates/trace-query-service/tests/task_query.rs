use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::util::ServiceExt;
use trace_core::{S3Grants, TaskCapabilityIssueRequest};
use trace_core::Signer as _;
use trace_core::lite::jwt::{Hs256TaskCapabilityConfig, TaskCapability};
use trace_query_service::{build_state, config::QueryServiceConfig, router, TASK_CAPABILITY_HEADER, TaskQueryRequest};
use uuid::Uuid;

fn issue_token(cfg: &QueryServiceConfig, task_id: Uuid, attempt: i64) -> anyhow::Result<String> {
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
        datasets: Vec::new(),
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

#[tokio::test]
async fn auth_required_missing_token() -> anyhow::Result<()> {
    let cfg = QueryServiceConfig::from_env()?;
    let state = build_state(cfg.clone()).await?;
    let app = router(state);

    let req = TaskQueryRequest {
        task_id: Uuid::new_v4(),
        attempt: 1,
        dataset_id: Uuid::new_v4(),
        sql: "SELECT 1".to_string(),
        limit: None,
    };

    let (status, _body) = send_query(app, None, &req).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn wrong_token_rejected() -> anyhow::Result<()> {
    let cfg = QueryServiceConfig::from_env()?;
    let state = build_state(cfg.clone()).await?;
    let app = router(state);

    let task_id = Uuid::new_v4();
    let token = issue_token(&cfg, Uuid::new_v4(), 1)?;

    let req = TaskQueryRequest {
        task_id,
        attempt: 1,
        dataset_id: Uuid::new_v4(),
        sql: "SELECT 1".to_string(),
        limit: None,
    };

    let (status, _body) = send_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn gate_rejects_unsafe_sql() -> anyhow::Result<()> {
    let cfg = QueryServiceConfig::from_env()?;
    let state = build_state(cfg.clone()).await?;
    let app = router(state);

    let task_id = Uuid::new_v4();
    let attempt = 1;
    let token = issue_token(&cfg, task_id, attempt)?;

    for sql in [
        "INSTALL httpfs",
        "SELECT * FROM read_csv('data')",
        "SELECT * FROM 'local.csv'",
    ] {
        let req = TaskQueryRequest {
            task_id,
            attempt,
            dataset_id: Uuid::new_v4(),
            sql: sql.to_string(),
            limit: None,
        };
        let (status, _body) = send_query(app.clone(), Some(token.clone()), &req).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "sql should be rejected: {sql}");
    }

    Ok(())
}

#[tokio::test]
async fn allows_url_literal_and_executes_selects() -> anyhow::Result<()> {
    let cfg = QueryServiceConfig::from_env()?;
    let state = build_state(cfg.clone()).await?;
    let app = router(state);

    let task_id = Uuid::new_v4();
    let attempt = 1;
    let token = issue_token(&cfg, task_id, attempt)?;

    // Allow URL strings as inert literals (not external reads).
    let req = TaskQueryRequest {
        task_id,
        attempt,
        dataset_id: Uuid::new_v4(),
        sql: "SELECT 'https://example.com'".to_string(),
        limit: None,
    };
    let (status, body) = send_query(app.clone(), Some(token.clone()), &req).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rows"][0][0].as_str(), Some("https://example.com"));

    // SELECT 1 returns a single row.
    let req = TaskQueryRequest {
        task_id,
        attempt,
        dataset_id: Uuid::new_v4(),
        sql: "SELECT 1".to_string(),
        limit: None,
    };
    let (status, body) = send_query(app.clone(), Some(token.clone()), &req).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rows"][0][0].as_i64(), Some(1));

    // Fixture table exists.
    let req = TaskQueryRequest {
        task_id,
        attempt,
        dataset_id: Uuid::new_v4(),
        sql: "SELECT id, message FROM alerts ORDER BY id LIMIT 5".to_string(),
        limit: None,
    };
    let (status, body) = send_query(app, Some(token), &req).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["truncated"].as_bool(), Some(false));
    assert_eq!(body["columns"][0]["name"], "id");
    assert_eq!(body["columns"][1]["name"], "message");
    assert_eq!(body["rows"].as_array().unwrap().len(), 5);
    assert_eq!(body["rows"][0][0].as_i64(), Some(1));
    assert_eq!(body["rows"][0][1].as_str(), Some("alert-1"));
    Ok(())
}
