use crate::config::{CompiledRule, IpMatch};
use anyhow::{anyhow, Result};
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

fn route_add(dest: Ipv4Addr, prefix_len: u8, gateway: Ipv4Addr, if_index: u32) -> Result<()> {
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

fn route_delete(dest: Ipv4Addr) {
    let _ = Command::new("route")
        .args(["delete", &dest.to_string()])
        .status();
}

pub fn install_routes_for_rules(rules: &[CompiledRule]) -> Result<Vec<InstalledRoute>> {
    let mut installed = Vec::new();

    for rule in rules {
        let (dest, prefix_len) = match &rule.match_pattern {
            IpMatch::Network(net) => match net.network() {
                IpAddr::V4(v4) => (v4, net.prefix()),
                _ => continue,
            },
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
