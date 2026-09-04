use crate::config::{CompiledRule, MatchPattern, RoutingRule, TargetKind, DestinationKind};
use anyhow::{anyhow, Result};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};
use std::process::Command;
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIpForwardTable2, MIB_IPFORWARD_TABLE2,
};
use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

/// A route row installed for the lifetime of the running router service.
#[derive(Debug, Clone)]
pub struct InstalledRoute {
    pub dest: Ipv4Addr,
    pub prefix_len: u8,
}

/// One IPv4 row from the live Windows forwarding table.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SystemRouteRow {
    pub dest: Ipv4Addr,
    pub prefix_len: u8,
    pub gateway: Ipv4Addr,
    pub if_index: u32,
}

fn is_loopback_prefix(dest: Ipv4Addr, prefix_len: u8) -> bool {
    if dest.is_loopback() {
        return true;
    }
    if prefix_len == 0 {
        return false;
    }
    let network = u32::from(dest);
    let mask = if prefix_len >= 32 {
        u32::MAX
    } else {
        u32::MAX << (32 - prefix_len)
    };
    (network & mask) >> 24 == 127
}

fn is_multicast_prefix(dest: Ipv4Addr, prefix_len: u8) -> bool {
    if dest.octets()[0] >= 224 && dest.octets()[0] <= 239 {
        return true;
    }
    prefix_len <= 4 && (u32::from(dest) & 0xF000_0000) == 0xE000_0000
}

pub fn system_row_to_rule(row: &SystemRouteRow) -> RoutingRule {
    let (target, target_value) = if row.prefix_len == 32 {
        (TargetKind::Ip, row.dest.to_string())
    } else {
        (
            TargetKind::Cidr,
            format!("{}/{}", row.dest, row.prefix_len),
        )
    };
    RoutingRule {
        target,
        target_value,
        destination: DestinationKind::Ip,
        destination_value: row.gateway.to_string(),
    }
}

pub fn rule_identity_key(rule: &RoutingRule) -> String {
    format!(
        "{}:{}|{}:{}",
        serde_json::to_value(rule.target)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "?".into()),
        rule.target_value.trim().to_ascii_lowercase(),
        serde_json::to_value(rule.destination)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "?".into()),
        rule.destination_value.trim()
    )
}

/// Read host routes already present in the Windows IPv4 forwarding table.
pub fn read_applied_ipv4_routes() -> Result<Vec<SystemRouteRow>> {
    unsafe {
        let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
        let rc = GetIpForwardTable2(AF_INET, &mut table);
        if rc != WIN32_ERROR(0) {
            return Err(anyhow!("GetIpForwardTable2 failed: {rc:?}"));
        }

        let table_ref = &*table;
        let rows = std::slice::from_raw_parts(
            table_ref.Table.as_ptr(),
            table_ref.NumEntries as usize,
        );

        let mut seen = HashSet::new();
        let mut routes = Vec::new();

        for row in rows {
            let prefix_len = row.DestinationPrefix.PrefixLength;
            if prefix_len == 0 {
                continue;
            }

            let dest = ipv4_from_sockaddr_in(&row.DestinationPrefix.Prefix.Ipv4);
            let gateway = ipv4_from_sockaddr_in(&row.NextHop.Ipv4);
            if gateway.is_unspecified() {
                continue;
            }
            if dest.is_unspecified() || is_loopback_prefix(dest, prefix_len) {
                continue;
            }
            if is_multicast_prefix(dest, prefix_len) {
                continue;
            }

            let entry = SystemRouteRow {
                dest,
                prefix_len,
                gateway,
                if_index: row.InterfaceIndex,
            };
            if seen.insert(entry.clone()) {
                routes.push(entry);
            }
        }

        FreeMibTable(table as *mut _);
        routes.sort_by(|a, b| {
            a.dest
                .octets()
                .cmp(&b.dest.octets())
                .then_with(|| a.prefix_len.cmp(&b.prefix_len))
                .then_with(|| a.gateway.octets().cmp(&b.gateway.octets()))
        });
        Ok(routes)
    }
}

/// Applied host routes that are not already represented in `config_rules`.
pub fn discover_external_routes(config_rules: &[RoutingRule]) -> Result<Vec<RoutingRule>> {
    let configured: HashSet<String> = config_rules.iter().map(rule_identity_key).collect();
    Ok(read_applied_ipv4_routes()?
        .into_iter()
        .map(|row| system_row_to_rule(&row))
        .filter(|rule| !configured.contains(&rule_identity_key(rule)))
        .collect())
}

fn ipv4_from_sockaddr_in(sin: &SOCKADDR_IN) -> Ipv4Addr {
    let raw = unsafe { sin.sin_addr.S_un.S_addr };
    Ipv4Addr::from_bits(u32::from_be(raw))
}

fn prefix_mask(prefix_len: u8) -> Ipv4Addr {
    if prefix_len == 0 {
        return Ipv4Addr::UNSPECIFIED;
    }
    if prefix_len >= 32 {
        return Ipv4Addr::new(255, 255, 255, 255);
    }
    let bits = u32::MAX << (32 - prefix_len);
    Ipv4Addr::from_bits(bits.to_be())
}

pub fn gateway_from_forward_table(if_index: u32) -> Result<Ipv4Addr> {
    unsafe {
        let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
        let rc = GetIpForwardTable2(AF_INET, &mut table);
        if rc != WIN32_ERROR(0) {
            return Err(anyhow!("GetIpForwardTable2 failed: {rc:?}"));
        }
        let table_ref = &*table;
        let mut best: Option<Ipv4Addr> = None;
        let rows = std::slice::from_raw_parts(
            table_ref.Table.as_ptr(),
            table_ref.NumEntries as usize,
        );
        for row in rows {
            if row.InterfaceIndex != if_index || row.DestinationPrefix.PrefixLength != 0 {
                continue;
            }
            let gw = ipv4_from_sockaddr_in(&row.NextHop.Ipv4);
            if !gw.is_unspecified() {
                best = Some(gw);
                break;
            }
        }
        FreeMibTable(table as *mut _);
        best.ok_or_else(|| anyhow!("no 0.0.0.0/0 route on interface index {if_index}"))
    }
}

/// True when a /32 (or any) host row for `dest` is already in the forwarding table.
pub fn host_route_exists(dest: Ipv4Addr) -> Result<bool> {
    Ok(read_applied_ipv4_routes()?
        .into_iter()
        .any(|row| row.dest == dest && row.prefix_len == 32))
}

pub fn route_add(dest: Ipv4Addr, prefix_len: u8, gateway: Ipv4Addr, if_index: u32) -> Result<()> {
    let mask = prefix_mask(prefix_len);
    let output = Command::new("route")
        .args([
            "add",
            &dest.to_string(),
            "mask",
            &mask.to_string(),
            &gateway.to_string(),
            "metric",
            "1",
            "IF",
            &if_index.to_string(),
        ])
        .output()
        .map_err(|e| anyhow!("failed to run route.exe: {e}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stderr.contains("already exists") || stdout.contains("already exists") {
        return Ok(());
    }

    Err(anyhow!(
        "route add {dest}/{prefix_len} via {gateway} IF {if_index} failed: {stderr}{stdout}"
    ))
}

pub fn route_delete(dest: Ipv4Addr) {
    let _ = Command::new("route")
        .args(["delete", &dest.to_string()])
        .status();
}

pub fn install_routes_for_rules(rules: &[CompiledRule]) -> Result<Vec<InstalledRoute>> {
    let mut installed = Vec::new();

    for rule in rules {
        let (dest, prefix_len) = match &rule.match_pattern {
            MatchPattern::Network(net) => match net.network() {
                IpAddr::V4(v4) => (v4, net.prefix()),
                _ => continue,
            },
            MatchPattern::Ip(ip) => (*ip, 32),
            MatchPattern::Interface(_) => continue,
        };

        if prefix_len == 0 {
            continue;
        }

        let gateway = rule.gateway;
        route_add(dest, prefix_len, gateway, rule.if_index)?;

        log::info!(
            "installed route {}/{} via {} (if_index={})",
            dest,
            prefix_len,
            gateway,
            rule.if_index
        );
        println!(
            "[roust] Route: {}/{} → gateway {} (if_index={})",
            dest, prefix_len, gateway, rule.if_index
        );

        installed.push(InstalledRoute { dest, prefix_len });
    }

    Ok(installed)
}

pub fn remove_installed_routes(routes: &[InstalledRoute]) {
    for route in routes {
        route_delete(route.dest);
        log::info!("removed route {}/{}", route.dest, route.prefix_len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DestinationKind, TargetKind};

    #[test]
    fn system_row_to_rule_uses_ip_for_host_route() {
        let row = SystemRouteRow {
            dest: Ipv4Addr::new(104, 19, 223, 79),
            prefix_len: 32,
            gateway: Ipv4Addr::new(10, 20, 9, 254),
            if_index: 6,
        };
        let rule = system_row_to_rule(&row);
        assert_eq!(rule.target, TargetKind::Ip);
        assert_eq!(rule.target_value, "104.19.223.79");
        assert_eq!(rule.destination, DestinationKind::Ip);
        assert_eq!(rule.destination_value, "10.20.9.254");
    }

    #[test]
    fn discover_external_routes_skips_configured_matches() {
        let configured = vec![RoutingRule {
            target: TargetKind::Cidr,
            target_value: "10.0.0.0/8".to_string(),
            destination: DestinationKind::Ip,
            destination_value: "10.20.9.254".to_string(),
        }];
        let external = discover_external_routes(&configured).unwrap();
        assert!(
            !external
                .iter()
                .any(|rule| rule.target_value == "10.0.0.0/8"),
            "configured route must not appear twice as system route"
        );
    }

    #[test]
    fn read_applied_ipv4_routes_on_windows() {
        let routes = read_applied_ipv4_routes().expect("forward table");
        assert!(
            !routes.is_empty(),
            "Windows host should expose at least one applied IPv4 route"
        );
    }
}
