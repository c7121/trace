use anyhow::Context;
use clap::{Parser, Subcommand};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::process::{Child, Command};

#[derive(Parser, Debug)]
#[command(name = "trace-lite")]
#[command(about = "Trace Lite local dev stack runner", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Subcommand, Debug)]
enum CommandKind {
    /// Start deps, run migrations, and run Lite services (foreground).
    Up,

    /// Stop deps (docker compose down). Does not manage any running local processes.
    Down,

    /// Apply a job spec YAML via `trace-dispatcher apply`.
    Apply {
        #[arg(long)]
        file: PathBuf,
    },

    /// Show job progress via `trace-dispatcher status`.
    Status {
        #[arg(long)]
        job: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let repo = find_repo_root().context("find repo root (run from inside the repo)")?;

    match cli.command {
        CommandKind::Up => cmd_up(&repo).await,
        CommandKind::Down => cmd_down(&repo).await,
        CommandKind::Apply { file } => cmd_apply(&repo, &file).await,
        CommandKind::Status { job } => cmd_status(&repo, job.as_deref()).await,
    }
}

async fn cmd_up(repo: &Path) -> anyhow::Result<()> {
    run_docker_compose(repo, &["up", "-d"])
        .await
        .context("docker compose up -d")?;

    cargo_build(
        repo,
        &["trace-harness", "trace-query-service", "trace-dispatcher"],
    )
    .await
    .context("cargo build required packages")?;

    run_migrations(repo).await.context("run migrations")?;

    let harness_bin = bin_path(repo, "trace-harness");
    let qs_bin = bin_path(repo, "trace-query-service");

    let mut dispatcher = spawn(&harness_bin, &["dispatcher"]).context("start dispatcher")?;
    let mut sink = spawn(&harness_bin, &["sink"]).context("start sink")?;
    let mut cryo_worker = spawn(&harness_bin, &["cryo-worker"]).context("start cryo worker")?;
    let mut query_service = spawn(&qs_bin, &[]).context("start query service")?;

    eprintln!(
        "\ntrace-lite up: stack running\n\
\n\
Next:\n\
  trace-lite apply --file docs/examples/chain_sync.monad_mainnet.yaml\n\
  trace-lite status --job <job_id>\n\
\n\
Ctrl-C stops local processes (deps stay up; run `trace-lite down` to stop them).\n"
    );

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            eprintln!("trace-lite up: ctrl-c received, stopping...");
        }
        status = dispatcher.wait() => {
            eprintln!("trace-lite up: dispatcher exited: {status:?}");
        }
        status = sink.wait() => {
            eprintln!("trace-lite up: sink exited: {status:?}");
        }
        status = cryo_worker.wait() => {
            eprintln!("trace-lite up: cryo-worker exited: {status:?}");
        }
        status = query_service.wait() => {
            eprintln!("trace-lite up: query-service exited: {status:?}");
        }
    }

    kill_and_wait("dispatcher", &mut dispatcher).await;
    kill_and_wait("sink", &mut sink).await;
    kill_and_wait("cryo-worker", &mut cryo_worker).await;
    kill_and_wait("query-service", &mut query_service).await;

    Ok(())
}

async fn cmd_down(repo: &Path) -> anyhow::Result<()> {
    run_docker_compose(repo, &["down"])
        .await
        .context("docker compose down")?;
    Ok(())
}

async fn cmd_apply(repo: &Path, file: &Path) -> anyhow::Result<()> {
    cargo_build(repo, &["trace-dispatcher"])
        .await
        .context("cargo build trace-dispatcher")?;

    let dispatcher_bin = bin_path(repo, "trace-dispatcher");
    run_bin(
        &dispatcher_bin,
        &["apply", "--file", file.to_string_lossy().as_ref()],
    )
    .await
    .with_context(|| format!("trace-dispatcher apply --file {}", file.display()))?;
    Ok(())
}

async fn cmd_status(repo: &Path, job: Option<&str>) -> anyhow::Result<()> {
    cargo_build(repo, &["trace-dispatcher"])
        .await
        .context("cargo build trace-dispatcher")?;

    let dispatcher_bin = bin_path(repo, "trace-dispatcher");
    let mut args: Vec<&str> = vec!["status"];
    if let Some(job) = job {
        args.push("--job");
        args.push(job);
    }

    run_bin(&dispatcher_bin, &args)
        .await
        .context("trace-dispatcher status")?;
    Ok(())
}

async fn run_migrations(repo: &Path) -> anyhow::Result<()> {
    let harness_bin = bin_path(repo, "trace-harness");

    // Retry because Postgres may be starting up even after `compose up -d`.
    let mut last_err: Option<anyhow::Error> = None;
    for _ in 0..20 {
        match run_bin(&harness_bin, &["migrate"]).await {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("migrations failed")))
}

fn spawn(bin: &Path, args: &[&str]) -> anyhow::Result<Child> {
    Command::new(bin)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .with_context(|| format!("spawn {}", bin.display()))
}

async fn kill_and_wait(name: &str, child: &mut Child) {
    if let Some(pid) = child.id() {
        eprintln!("trace-lite up: stopping {name} (pid {pid})");
    }

    let _ = child.kill().await;
    let _ = child.wait().await;
}

async fn cargo_build(repo: &Path, packages: &[&str]) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(repo).arg("build");
    for p in packages {
        cmd.arg("-p").arg(p);
    }
    run_cmd(&mut cmd).await.context("cargo build")?;
    Ok(())
}

async fn run_docker_compose(repo: &Path, args: &[&str]) -> anyhow::Result<()> {
    let mut cmd = Command::new("docker");
    cmd.current_dir(repo.join("harness"))
        .arg("compose")
        .args(args);
    run_cmd(&mut cmd).await.context("docker compose")?;
    Ok(())
}

async fn run_bin(bin: &Path, args: &[&str]) -> anyhow::Result<()> {
    let mut cmd = Command::new(bin);
    cmd.args(args);
    run_cmd(&mut cmd)
        .await
        .with_context(|| format!("run {}", bin.display()))
}

async fn run_cmd(cmd: &mut Command) -> anyhow::Result<()> {
    let status = cmd.status().await.context("spawn command")?;
    if !status.success() {
        anyhow::bail!("command failed: {status}");
    }
    Ok(())
}

fn bin_path(repo: &Path, name: &str) -> PathBuf {
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("target"));
    target_dir
        .join("debug")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

fn find_repo_root() -> anyhow::Result<PathBuf> {
    let mut dir = std::env::current_dir().context("read cwd")?;
    for _ in 0..8 {
        if dir.join("harness").join("docker-compose.yml").exists()
            && dir.join("docs").join("plan").join("milestones.md").exists()
        {
            return Ok(dir);
        }
        let Some(parent) = dir.parent() else {
            break;
        };
        dir = parent.to_path_buf();
    }
    anyhow::bail!("could not locate repo root (expected harness/docker-compose.yml)")
}
