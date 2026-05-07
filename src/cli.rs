use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "roust")]
#[command(about = "Windows 11 packet router - rule-based network interface routing", long_about = None)]
#[command(version)]
#[command(after_help = "EXAMPLES:
  roust nics show
    Display all network interfaces on this machine

  roust ip dest show --ip=192.168.1.100
    Check where 192.168.1.100 will be routed

  roust ip dest add --ip=192.168.1.0/24 --dest=Ethernet
    Route all IPs in 192.168.1.0/24 to Ethernet NIC

  roust ip dest add --file=ips.txt --dest=WiFi
    Import routes from a file (one IP/CIDR per line)

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
    #[command(about = "Query routing destination for an IP address")]
    Show {
        /// IP address to query (e.g., 192.168.1.100)
        #[arg(long)]
        ip: String,
    },

    /// List all configured routing rules
    #[command(about = "Display all active routing rules in order")]
    List,

    /// Add a routing rule
    #[command(about = "Add a new routing rule for IPs or CIDR ranges")]
    Add {
        /// IP address or CIDR range (e.g., 192.168.1.0/24, 192.168.1.100, or use --file)
        #[arg(long, help = "Single IP or CIDR range (e.g., 192.168.1.0/24)")]
        ip: Option<String>,

        /// Destination NIC name (use 'roust nics show' to list)
        #[arg(long, help = "NIC name to route to")]
        dest: String,

        /// File containing list of IPs (one per line, # for comments)
        #[arg(long, help = "Path to text file with IP list")]
        file: Option<PathBuf>,
    },

    /// Remove a routing rule
    #[command(about = "Delete a routing rule by IP address or CIDR range")]
    Remove {
        /// IP address or CIDR range to remove
        #[arg(long, help = "IP or CIDR to remove")]
        ip: String,
    },
}

pub fn parse_cli() -> Cli {
    Cli::parse()
}
