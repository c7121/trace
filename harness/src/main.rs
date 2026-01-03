use anyhow::Context;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod config;
mod migrate;

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

    /// Run the buffered sink consumer (stub in skeleton).
    Sink,
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
        Command::Dispatcher => {
            tracing::info!("dispatcher: not implemented in skeleton (see harness/AGENT_TASKS.md)");
            Ok(())
        }
        Command::Worker => {
            tracing::info!("worker: not implemented in skeleton (see harness/AGENT_TASKS.md)");
            Ok(())
        }
        Command::Sink => {
            tracing::info!("sink: not implemented in skeleton (see harness/AGENT_TASKS.md)");
            Ok(())
        }
    }
}
