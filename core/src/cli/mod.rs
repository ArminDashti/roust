use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "roust")]
#[command(about = "Windows 11 packet router - rule-based routing by interface default gateway", long_about = None)]
#[command(version)]
#[command(
    after_help = "EXAMPLES:  roust gateway list    List adapters with their MAC and gateway  roust rule list    List routing rules  roust add rule --ip 10.0.0.0/8 --mac AA:BB:CC:DD:EE:FF    Add a rule  roust add rule --file routes.json    Import rules  roust start    Start the router Windows service  roust stop    Stop the router Windows service  roust status    Show started/stopped state, install directory, and version"
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
    Gateway {
        #[command(subcommand)]
        action: GatewayCommands,
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
        /// Hardware MAC address of the target adapter (e.g. AA:BB:CC:DD:EE:FF).
        /// Highest priority — wins over --nic and --gateway when set.
        /// Run `roust gateway list` to see available MACs.
        #[arg(long)]
        mac: Option<String>,
        /// Friendly, display, or internal name of the target adapter (e.g. "Wi-Fi", "Ethernet").
        /// Used when --mac is not set; wins over --gateway.
        /// Run `roust gateway list` to see available names.
        #[arg(long)]
        nic: Option<String>,
        /// Default gateway IP of the target adapter (e.g. 192.168.1.1).
        /// Lowest priority — used only when neither --mac nor --nic is set.
        /// Run `roust gateway list` to see available gateways.
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

#[derive(Subcommand)]
pub enum GatewayCommands {
    /// List default gateways on local network interfaces.
    List,
}

pub fn parse_cli() -> Cli {
    Cli::parse()
}
