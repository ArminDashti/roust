use crate::config::MacEntry;
use anyhow::Result;
use std::collections::HashMap;
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
pub use routes::{
    gateway_from_forward_table, install_routes_for_rules, remove_installed_routes,
    InstalledRoute,
};
pub use win::{build_gateway_index_map, enumerate_interfaces, gateway_exists_on_host, predict_ipv4_egress};

/// Build MAC, NIC-name, and gateway-IP lookup maps from live adapter enumeration.
///
/// NIC name map is keyed by lowercase friendly name, display name, and internal name
/// so lookups are case-insensitive. Priority at rule-compile time: MAC > NIC > gateway.
pub fn build_adapter_maps(
    interfaces: &[NetworkInterface],
) -> (HashMap<String, MacEntry>, HashMap<String, MacEntry>, HashMap<Ipv4Addr, MacEntry>) {
    let mut mac_map = HashMap::new();
    let mut nic_map = HashMap::new();
    let mut gw_map = HashMap::new();
    for nic in interfaces {
        let gateway = nic
            .default_gateway
            .or_else(|| gateway_from_forward_table(nic.if_index).ok());
        let Some(gw) = gateway else { continue };
        let egress_ipv4 = nic
            .ipv4_address
            .as_ref()
            .and_then(|ip| ip.parse::<Ipv4Addr>().ok())
            .filter(|ip| !ip.is_unspecified() && !ip.is_loopback());
        let entry = MacEntry { if_index: nic.if_index, gateway: gw, egress_ipv4 };
        mac_map.insert(nic.mac_address.to_ascii_uppercase(), entry.clone());
        // Insert all name variants (lowercase) so lookups are case-insensitive.
        nic_map.insert(nic.name.to_ascii_lowercase(), entry.clone());
        nic_map.insert(nic.display_name.to_ascii_lowercase(), entry.clone());
        if let Some(ref friendly) = nic.friendly_name {
            nic_map.insert(friendly.to_ascii_lowercase(), entry.clone());
        }
        gw_map.insert(gw, entry);
    }
    (mac_map, nic_map, gw_map)
}

/// Compile routing rules against the current host interfaces.
pub fn build_compiled_rules(
    config: &crate::config::Config,
) -> Result<Vec<crate::config::CompiledRule>> {
    let interfaces = enumerate_interfaces()?;
    let (mac_map, nic_map, gw_map) = build_adapter_maps(&interfaces);
    config.compile_rules(&mac_map, &nic_map, &gw_map)
}

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
