mod daemon;
mod nodes;
mod proxy;
mod update;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nightshift-daemon", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon
    Daemon,
    /// Check for updates and apply if available
    Update,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("nightshift_daemon=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Daemon => daemon::run().await?,
        Command::Update => match update::check_and_apply().await? {
            true => tracing::info!("update applied successfully"),
            false => tracing::info!("already up to date"),
        },
    }

    Ok(())
}
