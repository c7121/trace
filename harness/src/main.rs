use anyhow::Context;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use trace_harness::{config, cryo_worker, dispatcher, enqueue, invoker, migrate, sink, worker};

#[derive(Parser, Debug)]
#[command(name = "trace-harness")]
#[command(about = "Trace contract-freeze harness (Trace Lite mode)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run DB migrations for state + data databases.
    Migrate,

    /// Run the Dispatcher HTTP server (stub in skeleton).
    Dispatcher,

    /// Run the worker wrapper (stub in skeleton).
    Worker,

    /// Run the Lite cryo_ingest worker (writes Parquet + registers dataset versions).
    CryoWorker,

    /// Run the local invoker + fake UDF runner.
    Invoker,

    /// Run the buffered sink consumer (stub in skeleton).
    Sink,

    /// Enqueue a task wakeup message (manual testing helper).
    Enqueue {
        /// Task id to enqueue (optional; generated if omitted).
        #[arg(long)]
        task_id: Option<uuid::Uuid>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let cfg = config::HarnessConfig::from_env().context("load harness config")?;

    match cli.command {
        Command::Migrate => migrate::run(&cfg).await,
        Command::Dispatcher => dispatcher::run(&cfg).await,
        Command::Worker => worker::run(&cfg).await,
        Command::CryoWorker => cryo_worker::run(&cfg).await,
        Command::Invoker => invoker::run(&cfg).await,
        Command::Sink => sink::run(&cfg).await,
        Command::Enqueue { task_id } => enqueue::run(&cfg, task_id).await,
    }
}
