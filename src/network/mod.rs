//! Network interface discovery and egress prediction (Windows routing table).

use std::net::Ipv4Addr;

/// Represents a network interface
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub display_name: String,
    pub if_index: u32,
    pub mac_address: String,
    pub ipv4_address: Option<String>,
    pub status: String,
}

/// Result of resolving which local interface Windows would use for an IPv4 destination.
#[derive(Debug, Clone)]
pub struct EgressPrediction {
    pub dest: Ipv4Addr,
    /// Interface index from the best route (`MIB_IPFORWARDROW::dwForwardIfIndex`).
    pub if_index: u32,
    /// Next hop from the routing table (may be `0.0.0.0` for on-link).
    pub next_hop: Ipv4Addr,
    /// Adapter name when matched to `GetAdaptersInfo` (`Index`).
    pub nic_name: Option<String>,
    pub nic_display: Option<String>,
}

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::{enumerate_interfaces, get_interface, interface_exists, predict_ipv4_egress};

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use stub::{enumerate_interfaces, predict_ipv4_egress};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_interfaces() {
        match enumerate_interfaces() {
            Ok(interfaces) => {
                println!("Found {} network interfaces", interfaces.len());
                for nic in &interfaces {
                    println!("  - {} ({})", nic.name, nic.display_name);
                }
                assert!(!interfaces.is_empty(), "Should have at least one NIC on Windows");
            }
            Err(e) => {
                println!("Error enumerating interfaces: {:?}", e);
            }
        }
    }
}
