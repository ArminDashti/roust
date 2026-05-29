use anyhow::Result;
use std::collections::HashMap;
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
pub use routes::{
    gateway_from_forward_table, install_routes_for_rules, remove_installed_routes,
    InstalledRoute,
};

/// Build gateway → `if_index` map from adapter gateways and the IPv4 forward table.
pub fn build_routing_gateway_index_map(
    interfaces: &[NetworkInterface],
) -> Result<std::collections::HashMap<Ipv4Addr, u32>> {
    let mut map = build_gateway_index_map(interfaces)?;
    for nic in interfaces {
        if let Ok(gw) = gateway_from_forward_table(nic.if_index) {
            win::insert_gateway_mapping(&mut map, gw, nic.if_index)?;
        }
    }
    Ok(map)
}
pub use win::{build_gateway_index_map, enumerate_interfaces, gateway_exists_on_host, predict_ipv4_egress};

/// Compile routing rules against the current host interfaces and gateway map.
pub fn build_compiled_rules(
    config: &crate::config::Config,
) -> Result<Vec<crate::config::CompiledRule>> {
    let interfaces = enumerate_interfaces()?;
    let gateway_index_map = build_routing_gateway_index_map(&interfaces)?;
    let mut ipv4_by_index = HashMap::new();

    for nic in &interfaces {
        if let Some(ip) = &nic.ipv4_address {
            if let Ok(addr) = ip.parse::<Ipv4Addr>() {
                if !addr.is_unspecified() && !addr.is_loopback() {
                    ipv4_by_index.insert(nic.if_index, addr);
                }
            }
        }
    }

    config.compile_rules(&gateway_index_map, &ipv4_by_index)
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
