use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Parser)]
#[command(name = "roust")]
#[command(about = "Windows 11 packet router - rule-based network interface routing", long_about = None)]
#[command(version)]
#[command(
    after_help = "EXAMPLES:  roust nics list    List network interfaces  roust route predict --dest 8.8.8.8    Predict egress NIC for a destination  roust add --file=routes.json    Import routing rules  roust service install    Register the Windows service (elevated)  roust start    Start the router Windows service  roust stop    Stop the router Windows service  roust status    Show Windows service state  roust update    Download Iran aggregated IP list files"
)]

pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    #[arg(global = true, long)]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    Add {
        #[command(subcommand)]
        action: RuleAction,
    },
    Delete {
        #[command(subcommand)]
        action: RuleAction,
    },
    Edit {
        #[command(subcommand)]
        action: RuleAction,
    },
    Start,
    Stop,
    Restart,
    Status,
    Service {
        #[command(subcommand)]
        action: ServiceCommands,
    },
    Nics {
        #[command(subcommand)]
        action: NicCommands,
    },
    Route {
        #[command(subcommand)]
        action: RouteCommands,
    },
    Update,
}

#[derive(Subcommand)]
pub enum RuleAction {
    Rule {
        #[arg(long)]
        ip: Option<String>,
        #[arg(long)]
        nic: Option<String>,
        #[arg(long)]
        rewrite_to: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum NicCommands {
    List,
}

#[derive(Subcommand)]
pub enum ServiceCommands {
    /// Register the roust Windows service (requires elevation).
    Install {
        /// Start automatically when Windows boots (default: manual start via `roust start`).
        #[arg(long)]
        auto: bool,
    },
    /// Remove the roust Windows service registration.
    Uninstall,
}

#[derive(Subcommand)]
pub enum RouteCommands {
    Predict {
        #[arg(long)]
        dest: String,
    },
}
pub fn parse_cli() -> Cli {
    Cli::parse()
}
