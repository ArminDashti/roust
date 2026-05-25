use std::net::Ipv4Addr;
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub display_name: String,
    /// Windows interface alias (e.g. `Ethernet`, `Wi-Fi`) from `GetAdaptersAddresses`.
    pub friendly_name: Option<String>,
    pub default_gateway: Option<Ipv4Addr>,
    pub if_index: u32,
    pub mac_address: String,
    pub ipv4_address: Option<String>,
    pub status: String,
}
#[derive(Debug, Clone)]
pub struct EgressPrediction {
    pub dest: Ipv4Addr,
    pub if_index: u32,
    pub next_hop: Ipv4Addr,
    pub nic_name: Option<String>,
    pub nic_display: Option<String>,
}

mod routes;
mod win;
pub use routes::{install_routes_for_rules, remove_installed_routes, InstalledRoute};
pub use win::{enumerate_interfaces, nic_name_matches, predict_ipv4_egress};

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
                assert!(
                    !interfaces.is_empty(),
                    "Should have at least one NIC on Windows"
                );
            }
            Err(e) => {
                println!("Error enumerating interfaces: {:?}", e);
            }
        }
    }
}
