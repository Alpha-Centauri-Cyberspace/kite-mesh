use std::path::PathBuf;

use clap::{Parser, Subcommand};
use kited::{Kited, config::KitedConfig, metrics};
use tokio::signal;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Debug, Parser)]
#[command(name = "kited", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Boot the daemon with the given config file.
    Run {
        #[arg(short, long, default_value = "kited.toml")]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run { config } => run(config).await,
    }
}

async fn run(config_path: PathBuf) -> anyhow::Result<()> {
    let cfg: KitedConfig = if config_path.exists() {
        let text = tokio::fs::read_to_string(&config_path).await?;
        toml::from_str(&text)?
    } else {
        tracing::warn!(?config_path, "config file not found; using defaults");
        KitedConfig {
            identity: Default::default(),
            network: Default::default(),
            storage: Default::default(),
            metrics: Default::default(),
        }
    };

    let metrics_handle = metrics::install()?;

    let handle = Kited::start(cfg, metrics_handle).await?;
    tracing::info!(peer_id = %handle.peer_id, "kited booted");

    signal::ctrl_c().await?;
    tracing::info!("shutting down");
    handle.inbox.shutdown().await;
    handle.join().await
}
