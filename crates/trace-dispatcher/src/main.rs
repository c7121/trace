use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use std::collections::VecDeque;
use trace_dispatcher::planner::{plan_chain_sync, PlanChainSyncRequest};

fn usage() -> &'static str {
    "usage: trace-dispatcher plan-chain-sync --chain-id <id> --to-block <exclusive> [--from-block <n>] [--chunk-size <n>] [--max-inflight <n>]\n\
env:\n\
  STATE_DATABASE_URL (default postgres://trace:trace@localhost:5433/trace_state)\n\
  TASK_WAKEUP_QUEUE  (default task_wakeup)\n"
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args: VecDeque<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.pop_front() else {
        eprintln!("{usage()}");
        return Ok(());
    };

    if cmd == "--help" || cmd == "-h" {
        println!("{usage()}");
        return Ok(());
    }

    match cmd.as_str() {
        "plan-chain-sync" => plan_chain_sync_cmd(args).await,
        _ => {
            eprintln!("unknown command: {cmd}\n\n{usage()}");
            Ok(())
        }
    }
}

async fn plan_chain_sync_cmd(args: VecDeque<String>) -> anyhow::Result<()> {
    let flags = parse_flags(args)?;

    let chain_id: i64 = required_i64(&flags, "chain-id")?;
    let to_block: i64 = required_i64(&flags, "to-block")?;
    let from_block: i64 = optional_i64(&flags, "from-block")?.unwrap_or(0);
    let chunk_size: i64 = optional_i64(&flags, "chunk-size")?.unwrap_or(1_000);
    let max_inflight: i64 = optional_i64(&flags, "max-inflight")?.unwrap_or(10);

    let state_database_url = std::env::var("STATE_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://trace:trace@localhost:5433/trace_state".to_string());
    let task_wakeup_queue =
        std::env::var("TASK_WAKEUP_QUEUE").unwrap_or_else(|_| "task_wakeup".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&state_database_url)
        .await
        .context("connect state db")?;

    let res = plan_chain_sync(
        &pool,
        &task_wakeup_queue,
        PlanChainSyncRequest {
            chain_id,
            from_block,
            to_block,
            chunk_size,
            max_inflight,
        },
    )
    .await?;

    println!(
        "scheduled_ranges={} next_block={}",
        res.scheduled_ranges, res.next_block
    );
    Ok(())
}

fn parse_flags(mut args: VecDeque<String>) -> anyhow::Result<std::collections::HashMap<String, String>>
{
    let mut out = std::collections::HashMap::<String, String>::new();
    while let Some(arg) = args.pop_front() {
        if !arg.starts_with("--") {
            anyhow::bail!("unexpected arg: {arg}\n\n{}", usage());
        }

        let without = arg.trim_start_matches("--");
        if let Some((k, v)) = without.split_once('=') {
            out.insert(k.to_string(), v.to_string());
            continue;
        }

        let Some(value) = args.pop_front() else {
            anyhow::bail!("missing value for {arg}\n\n{}", usage());
        };
        out.insert(without.to_string(), value);
    }
    Ok(out)
}

fn required_i64(flags: &std::collections::HashMap<String, String>, key: &str) -> anyhow::Result<i64> {
    let Some(v) = flags.get(key) else {
        anyhow::bail!("missing --{key}\n\n{}", usage());
    };
    v.parse::<i64>()
        .with_context(|| format!("parse --{key}={v}"))
}

fn optional_i64(
    flags: &std::collections::HashMap<String, String>,
    key: &str,
) -> anyhow::Result<Option<i64>> {
    let Some(v) = flags.get(key) else {
        return Ok(None);
    };
    Ok(Some(
        v.parse::<i64>()
            .with_context(|| format!("parse --{key}={v}"))?,
    ))
}

