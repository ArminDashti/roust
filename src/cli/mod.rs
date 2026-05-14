use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "roust")]
#[command(about = "Windows 11 packet router - rule-based network interface routing", long_about = None)]
#[command(version)]
#[command(after_help = "EXAMPLES:
  roust nics show
    Display all network interfaces on this machine

  roust route predict --dest 8.8.8.8
    Show which NIC Windows would use for that destination (routing table)

  roust ip dest show --ip=192.168.1.100
    Check where 192.168.1.100 will be routed

  roust ip dest add --ip=192.168.1.0/24 --dest=Ethernet
    Route all IPs in 192.168.1.0/24 to Ethernet NIC

  roust ip dest add --file=private_ips.json --dest=WiFi
    Import routes from a JSON or text file

  roust ip dest list
    Show all configured routing rules

  roust ip dest remove --ip=192.168.1.0/24
    Delete a routing rule

  roust start
    Start the router daemon

  roust stop
    Stop the router daemon
")]
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
    /// Add a new routing rule
    Add {
        #[command(subcommand)]
        action: RuleAction,
    },
    /// Delete a routing rule
    Delete {
        #[command(subcommand)]
        action: RuleAction,
    },
    /// Edit an existing routing rule
    Edit {
        #[command(subcommand)]
        action: RuleAction,
    },
    /// Start the router service
    Start,
    /// Stop the router service
    Stop,
    /// Restart the router service
    Restart,
    /// Display status of the router service
    Status,
    /// List network interfaces
    Nics {
        #[command(subcommand)]
        action: NicCommands,
    },
    /// Resolve egress interface from the routing table (before packet capture)
    Route {
        #[command(subcommand)]
        action: RouteCommands,
    },
}

#[derive(Subcommand)]
pub enum RuleAction {
    Rule {
        #[arg(long)]
        ip: Option<String>,
        #[arg(long)]
        nic: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum NicCommands {
    List,
}

#[derive(Subcommand)]
pub enum RouteCommands {
    /// Which NIC `GetBestRoute` selects for an IPv4 destination
    Predict {
        /// IPv4 address (e.g. 8.8.8.8)
        #[arg(long)]
        dest: String,
    },
}

pub fn parse_cli() -> Cli {
    Cli::parse()
}
