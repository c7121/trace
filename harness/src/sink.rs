use crate::{config::HarnessConfig, pgqueue::PgQueue, s3::ObjectStore};
use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use trace_core::{ObjectStore as ObjectStoreTrait, Queue as QueueTrait};
use trace_sink::{Sink, SinkConfig};

pub async fn run(cfg: &HarnessConfig) -> anyhow::Result<()> {
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

    let queue: Arc<dyn QueueTrait> = Arc::new(PgQueue::new(state_pool));
    let object_store: Arc<dyn ObjectStoreTrait> =
        Arc::new(ObjectStore::new(&cfg.s3_endpoint).context("init object store")?);

    let sink_cfg = SinkConfig {
        buffer_queue: cfg.buffer_queue.clone(),
        buffer_queue_dlq: cfg.buffer_queue_dlq.clone(),
        poll_interval: Duration::from_millis(cfg.sink_poll_ms),
        visibility_timeout: Duration::from_secs(cfg.sink_visibility_timeout_secs),
        retry_delay: Duration::from_millis(cfg.sink_retry_delay_ms),
        max_deliveries: cfg.sink_max_deliveries,
    };

    let sink = Sink::new(sink_cfg, queue, object_store, data_pool);

    tracing::info!(
        event = "harness.sink.started",
        queue = %cfg.buffer_queue,
        "sink started"
    );

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!(event = "harness.sink.shutdown", "sink shutting down");
            Ok(())
        }
        res = sink.run() => {
            res
        }
    }
}
