use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Parser)]
#[command(name = "roust")]
#[command(about = "Windows 11 packet router - rule-based network interface routing", long_about = None)]
#[command(version)]
#[command(
    after_help = "EXAMPLES:  roust nics show    Display all network interfaces on this machine  roust route predict --dest 8.8.8.8    Show which NIC Windows would use for that destination (routing table)  roust ip dest show --ip=192.168.1.100    Check where 192.168.1.100 will be routed  roust ip dest add --ip=192.168.1.0/24 --dest=Ethernet --rewrite-to=10.0.0.1    Match that range and rewrite IPv4 destination to 10.0.0.1 on reinject  roust ip dest add --ip=192.168.1.0/24 --dest=Ethernet    Route all IPs in 192.168.1.0/24 to Ethernet NIC (no destination rewrite)  roust add --file=routes.json    Import routes.json entries (each object has ip and nic fields)  roust add --file=private_ips.json --nic WiFi    Import CIDR strings from a JSON/text file using one NIC for all entries  roust ip dest list    Show all configured routing rules  roust ip dest remove --ip=192.168.1.0/24    Delete a routing rule  roust start    Start the router daemon  roust stop    Stop the router daemon  roust update    Download Iran aggregated IP blocks and write ipv4.txt, ipv6.txt, etc."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    #[arg(global = true, long)]
    pub config: Option<PathBuf>,
    #[arg(global = true, short, long)]
    pub verbose: bool,
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
pub enum RouteCommands {
    Predict {
        #[arg(long)]
        dest: String,
    },
}
pub fn parse_cli() -> Cli {
    Cli::parse()
}
