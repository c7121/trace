use crate::config::HarnessConfig;
use crate::jwt::{Hs256TaskCapabilityConfig, TaskCapability};
use anyhow::Context;
use sqlx::PgPool;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use trace_core::fixtures::{
    ALERTS_FIXTURE_DATASET_ID, ALERTS_FIXTURE_DATASET_STORAGE_PREFIX,
    ALERTS_FIXTURE_DATASET_VERSION,
};
use trace_core::{DatasetGrant, DatasetStorageRef, S3Grants, Signer as SignerTrait};

#[derive(Debug)]
pub struct DispatcherServer {
    pub addr: SocketAddr,
    inner: trace_dispatcher::DispatcherServer,
}

impl DispatcherServer {
    pub async fn start(
        pool: PgPool,
        cfg: HarnessConfig,
        bind: SocketAddr,
        enable_outbox: bool,
        enable_chain_sync_planner: bool,
        enable_lease_reaper: bool,
    ) -> anyhow::Result<Self> {
        let capability = TaskCapability::from_hs256_config(Hs256TaskCapabilityConfig {
            issuer: cfg.task_capability_iss.clone(),
            audience: cfg.task_capability_aud.clone(),
            current_kid: cfg.task_capability_kid.clone(),
            current_secret: cfg.task_capability_secret.clone(),
            next_kid: cfg.task_capability_next_kid.clone(),
            next_secret: cfg.task_capability_next_secret.clone(),
            ttl: Duration::from_secs(cfg.task_capability_ttl_secs),
        })
        .context("init task capability")?;

        let queue = Arc::new(crate::pgqueue::PgQueue::new(pool.clone()));

        let (bucket, prefix) =
            trace_core::lite::s3::parse_s3_uri(ALERTS_FIXTURE_DATASET_STORAGE_PREFIX)
                .context("parse fixture dataset storage prefix")?;

        let dispatcher_cfg = trace_dispatcher::DispatcherConfig {
            org_id: cfg.org_id,
            lease_duration_secs: cfg.lease_duration_secs,
            outbox_poll_ms: cfg.outbox_poll_ms,
            lease_reaper_poll_ms: cfg.lease_reaper_poll_ms,
            outbox_batch_size: cfg.outbox_batch_size,
            task_wakeup_queue: cfg.task_wakeup_queue.clone(),
            buffer_queue: cfg.buffer_queue.clone(),
            default_datasets: vec![DatasetGrant {
                dataset_uuid: ALERTS_FIXTURE_DATASET_ID,
                dataset_version: ALERTS_FIXTURE_DATASET_VERSION,
                storage_ref: Some(DatasetStorageRef::S3 {
                    bucket,
                    prefix,
                    glob: "*.parquet".to_string(),
                }),
            }],
            default_s3: S3Grants {
                read_prefixes: vec![ALERTS_FIXTURE_DATASET_STORAGE_PREFIX.to_string()],
                write_prefixes: Vec::new(),
            },
        };

        let inner = trace_dispatcher::DispatcherServer::start(
            pool,
            dispatcher_cfg,
            Arc::new(capability) as Arc<dyn SignerTrait>,
            queue,
            bind,
            enable_outbox,
            enable_chain_sync_planner,
            enable_lease_reaper,
        )
        .await?;

        Ok(Self {
            addr: inner.addr,
            inner,
        })
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.inner.shutdown().await
    }
}

pub async fn run(cfg: &HarnessConfig) -> anyhow::Result<()> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&cfg.state_database_url)
        .await
        .context("connect state db")?;

    let bind: SocketAddr = cfg
        .dispatcher_bind
        .parse()
        .with_context(|| format!("parse DISPATCHER_BIND={}", cfg.dispatcher_bind))?;

    let server = DispatcherServer::start(pool, cfg.clone(), bind, true, true, true).await?;
    tracing::info!(
        event = "harness.dispatcher.listening",
        addr = %server.addr,
        "dispatcher listening"
    );

    tokio::signal::ctrl_c().await.context("wait for ctrl-c")?;
    tracing::info!(
        event = "harness.dispatcher.shutdown",
        "dispatcher shutting down"
    );
    server.shutdown().await?;
    Ok(())
}
