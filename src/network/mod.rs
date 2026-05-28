use std::net::Ipv4Addr;

#[derive(Debug, Clone)]
pub struct NetworkInterface {
    /// Internal adapter name (GUID-style string from the OS).
    pub name: String,
    /// Hardware/driver description from the OS.
    pub display_name: String,
    /// Windows interface alias (e.g. `Ethernet`, `Wi-Fi`) from `GetAdaptersAddresses`.
    pub friendly_name: Option<String>,
    pub default_gateway: Option<Ipv4Addr>,
    pub if_index: u32,
    pub mac_address: String,
    pub ipv4_address: Option<String>,
    pub status: String,
}

impl NetworkInterface {
    /// Match a rule NIC field against internal name, description, or friendly name.
    pub fn matches_alias(&self, nic: &str) -> bool {
        self.name.eq_ignore_ascii_case(nic)
            || self.display_name.eq_ignore_ascii_case(nic)
            || self
                .friendly_name
                .as_deref()
                .is_some_and(|alias| alias.eq_ignore_ascii_case(nic))
    }
}

pub fn find_interface<'a>(
    interfaces: &'a [NetworkInterface],
    nic: &str,
) -> Option<&'a NetworkInterface> {
    interfaces.iter().find(|iface| iface.matches_alias(nic))
}

#[derive(Debug, Clone)]
pub struct EgressPrediction {
    pub dest: Ipv4Addr,
    pub if_index: u32,
    pub next_hop: Ipv4Addr,
    pub nic_name: Option<String>,
    pub nic_display: Option<String>,
    pub nic_friendly: Option<String>,
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
                    let friendly = nic.friendly_name.as_deref().unwrap_or("-");
                    println!(
                        "  - {} / {} ({})",
                        friendly, nic.name, nic.display_name
                    );
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
