use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "roust")]
#[command(about = "Windows 11 packet router - rule-based network interface routing", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Path to config file (defaults to roust.json or %ProgramData%\roust\config.json)
    #[arg(global = true, long)]
    pub config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(global = true, short, long)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage NIC (Network Interface Card) operations
    Nics {
        #[command(subcommand)]
        action: NicCommands,
    },

    /// Manage IP to destination routing rules
    Ip {
        #[command(subcommand)]
        action: IpCommands,
    },

    /// Start the router service
    Start {
        /// Run as a Windows Service instead of daemon
        #[arg(long)]
        service: bool,

        /// Automatically install as Windows Service if not already installed
        #[arg(long)]
        install_service: bool,
    },

    /// Stop the router service
    Stop {
        /// Stop the Windows Service instead of daemon
        #[arg(long)]
        service: bool,

        /// Uninstall the Windows Service
        #[arg(long)]
        uninstall_service: bool,
    },
}

#[derive(Subcommand)]
pub enum NicCommands {
    /// Display all network interfaces
    Show,
}

#[derive(Subcommand)]
pub enum IpCommands {
    /// Show destination for an IP address
    Dest {
        #[command(subcommand)]
        action: IpDestCommands,
    },
}

#[derive(Subcommand)]
pub enum IpDestCommands {
    /// Show where an IP address is routed
    Show {
        /// IP address to query (e.g., 192.168.1.100)
        #[arg(long)]
        ip: String,
    },

    /// Add a routing rule
    Add {
        /// IP address or CIDR range (e.g., 192.168.1.0/24, or path to .txt file with --file)
        #[arg(long)]
        ip: Option<String>,

        /// Destination NIC name
        #[arg(long)]
        dest: String,

        /// File containing list of IPs (one per line)
        #[arg(long)]
        file: Option<PathBuf>,
    },
}

pub fn parse_cli() -> Cli {
    Cli::parse()
}
