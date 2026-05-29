use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "roust")]
#[command(about = "Windows 11 packet router - rule-based routing by interface default gateway", long_about = None)]
#[command(version)]
#[command(
    after_help = "EXAMPLES:  roust rule list    List routing rules in routes.json  roust predict --ip 8.8.8.8    Predict egress for a destination  roust add rule --ip 10.0.0.0/8 --gateway 192.168.1.1    Add a rule  roust add rule --file routes.json    Import rules  roust start    Start the router Windows service  roust stop    Stop the router Windows service  roust status    Show started/stopped state, install directory, and version"
)]

pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
    #[arg(global = true, long)]
    pub config: Option<PathBuf>,
    /// Register the Windows service (installer / automation only).
    #[arg(long, hide = true)]
    pub install_service: bool,
    /// Remove the Windows service registration (installer / automation only).
    #[arg(long, hide = true)]
    pub uninstall_service: bool,
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
    Rule {
        #[command(subcommand)]
        action: RuleCommands,
    },
    Predict {
        #[arg(long)]
        ip: String,
    },
    Start,
    Stop,
    Restart,
    Status,
}

#[derive(Subcommand)]
pub enum RuleAction {
    Rule {
        #[arg(long)]
        ip: Option<String>,
        #[arg(long)]
        gateway: Option<String>,
        #[arg(long)]
        rewrite_to: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum RuleCommands {
    /// List all routing rules from routes.json.
    List,
}

pub fn parse_cli() -> Cli {
    Cli::parse()
}
