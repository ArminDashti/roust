use clap::Parser;
use roust::api::{self, ApiOptions};
use roust::elevation;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "roust-api")]
#[command(about = "Management HTTP API for roust service status and routes")]
struct Cli {
    /// Bind address for the HTTP server
    #[arg(long, default_value = "127.0.0.1:8787")]
    bind: String,

    /// Path to routes.json (defaults to ProgramData/roust or cwd)
    #[arg(long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Service control, host routes, and WFP require Administrator.
    elevation::ensure_elevated()?;

    env_logger::Builder::from_default_env()
        .format_timestamp_secs()
        .init();

    let cli = Cli::parse();
    api::serve(ApiOptions {
        bind: cli.bind,
        config_path: cli.config,
    })
    .await
}
