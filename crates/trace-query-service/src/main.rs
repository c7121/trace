use anyhow::Context;
use clap::Parser;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;
use trace_query_service::{config::QueryServiceConfig, build_state, router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,trace_query_service=debug")),
        )
        .init();

    let cfg = QueryServiceConfig::parse();
    let addr: SocketAddr = cfg.bind.parse().context("parse bind addr")?;

    let state = build_state(cfg).await.context("build state")?;
    let app = router(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("bind tcp listener")?;
    let local = listener.local_addr().context("read local addr")?;
    tracing::info!(addr = %local, "query service listening");

    axum::serve(listener, app)
        .await
        .context("serve query service")?;
    Ok(())
}

