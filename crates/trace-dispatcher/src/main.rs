use anyhow::Context;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::collections::VecDeque;
use trace_dispatcher::chain_sync::apply_chain_sync_yaml;
use uuid::Uuid;

fn usage() -> &'static str {
    "usage:\n\
  trace-dispatcher apply --file <path>\n\
  trace-dispatcher status [--job <job_id>]\n\
\
  # Back-compat alias (deprecated):\n\
  trace-dispatcher chain-sync apply --file <path>\n\
\
env:\n\
  ORG_ID             (default 00000000-0000-0000-0000-000000000001)\n\
  STATE_DATABASE_URL (default postgres://trace:trace@localhost:5433/trace_state)\n\
"
}

#[derive(Debug, Deserialize)]
struct SpecKind {
    kind: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args: VecDeque<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.pop_front() else {
        eprintln!("{}", usage());
        return Ok(());
    };

    if cmd == "--help" || cmd == "-h" {
        println!("{}", usage());
        return Ok(());
    }

    match cmd.as_str() {
        "apply" => apply_cmd(args).await,
        "status" => status_cmd(args).await,
        // NOTE: kept for back-compat / easy muscle memory; prefer `apply`.
        "chain-sync" => chain_sync_cmd(args).await,
        _ => {
            eprintln!("unknown command: {cmd}\n\n{}", usage());
            Ok(())
        }
    }
}

async fn status_cmd(args: VecDeque<String>) -> anyhow::Result<()> {
    let flags = parse_flags(args)?;

    let state_database_url = std::env::var("STATE_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://trace:trace@localhost:5433/trace_state".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&state_database_url)
        .await
        .context("connect state db")?;

    if let Some(job) = optional_string(&flags, "job") {
        let job_id = job.parse::<Uuid>().context("parse --job")?;
        let status = trace_dispatcher::status::fetch_chain_sync_status(&pool, job_id).await?;
        println!("{}", serde_json::to_string_pretty(&status)?);
        return Ok(());
    }

    let job_ids = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT job_id
        FROM state.chain_sync_jobs
        ORDER BY updated_at DESC
        "#,
    )
    .fetch_all(&pool)
    .await
    .context("list chain_sync jobs")?;

    let mut out = Vec::with_capacity(job_ids.len());
    for job_id in job_ids {
        out.push(trace_dispatcher::status::fetch_chain_sync_status(&pool, job_id).await?);
    }
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

async fn apply_cmd(args: VecDeque<String>) -> anyhow::Result<()> {
    let flags = parse_flags(args)?;
    let file = required_string(&flags, "file")?;

    let yaml = std::fs::read_to_string(&file).with_context(|| format!("read {file}"))?;

    // Determine spec kind from YAML and route.
    let kind = serde_yaml::from_str::<SpecKind>(&yaml)
        .with_context(|| format!("parse {file} (expected top-level `kind:`)"))?;

    let org_id = std::env::var("ORG_ID")
        .unwrap_or_else(|_| "00000000-0000-0000-0000-000000000001".to_string())
        .parse::<Uuid>()
        .context("parse ORG_ID")?;

    let state_database_url = std::env::var("STATE_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://trace:trace@localhost:5433/trace_state".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&state_database_url)
        .await
        .context("connect state db")?;

    match kind.kind.as_str() {
        "chain_sync" => {
            let res = apply_chain_sync_yaml(&pool, org_id, &yaml).await?;
            println!("job_id={}", res.job_id);
            Ok(())
        }
        other => {
            anyhow::bail!(
                "unsupported spec kind `{other}` (only `chain_sync` is supported today)\n\n{}",
                usage()
            );
        }
    }
}

async fn chain_sync_cmd(mut args: VecDeque<String>) -> anyhow::Result<()> {
    let Some(sub) = args.pop_front() else {
        eprintln!("{}", usage());
        return Ok(());
    };

    match sub.as_str() {
        // Back-compat alias; delegate to the generic router.
        "apply" => {
            eprintln!(
                "warning: `trace-dispatcher chain-sync apply` is deprecated; use `trace-dispatcher apply --file <path>`"
            );
            apply_cmd(args).await
        }
        _ => {
            eprintln!("unknown chain-sync command: {sub}\n\n{}", usage());
            Ok(())
        }
    }
}

fn parse_flags(
    mut args: VecDeque<String>,
) -> anyhow::Result<std::collections::HashMap<String, String>> {
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

fn required_string(
    flags: &std::collections::HashMap<String, String>,
    key: &str,
) -> anyhow::Result<String> {
    let Some(v) = flags.get(key) else {
        anyhow::bail!("missing --{key}\n\n{}", usage());
    };
    Ok(v.to_string())
}

fn optional_string(flags: &std::collections::HashMap<String, String>, key: &str) -> Option<String> {
    flags.get(key).map(|v| v.to_string())
}
