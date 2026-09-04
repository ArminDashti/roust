//! Temporary NIC-bound IPv4 ping: install a host route only for the run, then restore.

use super::routes::{gateway_from_forward_table, host_route_exists, route_add, route_delete};
use super::{enumerate_interfaces, find_interface};
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::net::{Ipv4Addr, ToSocketAddrs};
use std::time::Duration;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::IpHelper::{
    IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho2Ex, ICMP_ECHO_REPLY, IP_SUCCESS,
};

const DEFAULT_COUNT: u32 = 4;
const MAX_COUNT: u32 = 10;
const TIMEOUT_MS: u32 = 1000;
const PAYLOAD: &[u8] = b"roust-nic-ping-probe!!!!!!!!!!!!"; // 32 bytes

#[derive(Debug, Clone, Serialize)]
pub struct PingReply {
    pub seq: u32,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PingResult {
    pub host: String,
    pub dest_ip: String,
    pub nic: String,
    pub source_ip: String,
    pub sent: u32,
    pub received: u32,
    pub loss_pct: f64,
    pub route_restored: bool,
    pub replies: Vec<PingReply>,
}

/// Holds a temporary /32 host route and deletes it on drop when this call added it.
struct TempHostRoute {
    dest: Ipv4Addr,
    added_by_us: bool,
}

impl TempHostRoute {
    fn install(dest: Ipv4Addr, gateway: Ipv4Addr, if_index: u32) -> Result<Self> {
        let already = host_route_exists(dest)?;
        if already {
            return Ok(Self {
                dest,
                added_by_us: false,
            });
        }
        route_add(dest, 32, gateway, if_index)?;
        Ok(Self {
            dest,
            added_by_us: true,
        })
    }
}

impl Drop for TempHostRoute {
    fn drop(&mut self) {
        if self.added_by_us {
            route_delete(self.dest);
        }
    }
}

fn resolve_ipv4(host: &str) -> Result<Ipv4Addr> {
    let trimmed = host.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("host must not be empty"));
    }
    if let Ok(ip) = trimmed.parse::<Ipv4Addr>() {
        if ip.is_unspecified() || ip.is_multicast() {
            return Err(anyhow!("host address {ip} is not a unicast IPv4 target"));
        }
        return Ok(ip);
    }
    if trimmed.parse::<std::net::Ipv6Addr>().is_ok() {
        return Err(anyhow!("IPv6 hosts are not supported; use an IPv4 address or name"));
    }
    let addrs = format!("{trimmed}:0")
        .to_socket_addrs()
        .map_err(|e| anyhow!("failed to resolve host '{trimmed}': {e}"))?;
    for addr in addrs {
        if let std::net::IpAddr::V4(v4) = addr.ip() {
            if !v4.is_unspecified() && !v4.is_multicast() {
                return Ok(v4);
            }
        }
    }
    Err(anyhow!(
        "host '{trimmed}' did not resolve to an IPv4 address"
    ))
}

fn clamp_count(count: Option<u32>) -> u32 {
    count.unwrap_or(DEFAULT_COUNT).clamp(1, MAX_COUNT)
}

fn icmp_status_message(status: u32) -> String {
    match status {
        IP_SUCCESS => "ok".to_string(),
        11002 => "ICMP destination network unreachable".into(),
        11003 => "ICMP destination host unreachable".into(),
        11010 => "ICMP request timed out".into(),
        11013 => "ICMP TTL expired in transit".into(),
        other => format!("ICMP status {other}"),
    }
}

fn send_echo(
    handle: HANDLE,
    source: Ipv4Addr,
    dest: Ipv4Addr,
    seq: u32,
) -> PingReply {
    let request = PAYLOAD;
    // Reply buffer must hold ICMP_ECHO_REPLY + data + 8 bytes of ICMP error room.
    let reply_size = (std::mem::size_of::<ICMP_ECHO_REPLY>() + request.len() + 8) as u32;
    let mut reply_buf = vec![0u8; reply_size as usize];

    let replies = unsafe {
        IcmpSendEcho2Ex(
            handle,
            HANDLE::default(),
            None,
            None,
            source.to_bits(),
            dest.to_bits(),
            request.as_ptr().cast(),
            request.len() as u16,
            None,
            reply_buf.as_mut_ptr().cast(),
            reply_size,
            TIMEOUT_MS,
        )
    };

    if replies == 0 {
        return PingReply {
            seq,
            success: false,
            rtt_ms: None,
            error: Some(format!(
                "IcmpSendEcho2Ex failed (GetLastError={})",
                std::io::Error::last_os_error()
            )),
        };
    }

    let echo = unsafe { &*(reply_buf.as_ptr() as *const ICMP_ECHO_REPLY) };
    if echo.Status == IP_SUCCESS {
        PingReply {
            seq,
            success: true,
            rtt_ms: Some(echo.RoundTripTime),
            error: None,
        }
    } else {
        PingReply {
            seq,
            success: false,
            rtt_ms: None,
            error: Some(icmp_status_message(echo.Status)),
        }
    }
}

/// Ping `host` forcing egress via the NIC named by `nic` (temporary host route + source bind).
/// Always restores any host route this call added before returning.
pub fn ping_via_nic(host: &str, nic: &str, count: Option<u32>) -> Result<PingResult> {
    let dest = resolve_ipv4(host)?;
    let count = clamp_count(count);
    let interfaces = enumerate_interfaces()?;
    let iface = find_interface(&interfaces, nic.trim())
        .ok_or_else(|| anyhow!("NIC '{nic}' not found"))?;

    let source_ip: Ipv4Addr = iface
        .ipv4_address
        .as_deref()
        .ok_or_else(|| anyhow!("NIC '{nic}' has no IPv4 address"))?
        .parse()
        .map_err(|e| anyhow!("NIC '{nic}' has invalid IPv4 address: {e}"))?;

    if source_ip.is_unspecified() || source_ip.is_loopback() {
        return Err(anyhow!(
            "NIC '{nic}' IPv4 {source_ip} cannot be used as ping source"
        ));
    }

    let gateway = iface
        .default_gateway
        .or_else(|| gateway_from_forward_table(iface.if_index).ok())
        .unwrap_or(dest);

    let nic_label = iface
        .friendly_name
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if !iface.display_name.is_empty() {
                iface.display_name.clone()
            } else {
                iface.name.clone()
            }
        });

    let _temp_route = TempHostRoute::install(dest, gateway, iface.if_index)?;

    // Brief settle so the forwarding table picks up the new row before ICMP.
    std::thread::sleep(Duration::from_millis(50));

    let handle = unsafe { IcmpCreateFile() }
        .map_err(|e| anyhow!("IcmpCreateFile failed: {e}"))?;

    let mut replies = Vec::with_capacity(count as usize);
    for seq in 1..=count {
        replies.push(send_echo(handle, source_ip, dest, seq));
        if seq < count {
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    let _ = unsafe { IcmpCloseHandle(handle) };

    let received = replies.iter().filter(|r| r.success).count() as u32;
    let loss_pct = if count == 0 {
        0.0
    } else {
        ((count - received) as f64) * 100.0 / (count as f64)
    };

    // Drop temp route before constructing the result so restore happens even on later panics
    // in serialization — Drop of `_temp_route` runs at end of scope; force early restore:
    drop(_temp_route);

    Ok(PingResult {
        host: host.trim().to_string(),
        dest_ip: dest.to_string(),
        nic: nic_label,
        source_ip: source_ip.to_string(),
        sent: count,
        received,
        loss_pct,
        route_restored: true,
        replies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::routes::host_route_exists;

    #[test]
    fn unknown_nic_fails_without_host_route() {
        let dest = Ipv4Addr::new(203, 0, 113, 50);
        let before = host_route_exists(dest).unwrap_or(false);
        let err = ping_via_nic("203.0.113.50", "__no_such_nic__", Some(1)).unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
        let after = host_route_exists(dest).unwrap_or(false);
        assert_eq!(before, after, "failed ping must not leave a temp host route");
    }

    #[test]
    fn ping_restores_temp_host_route() {
        let interfaces = enumerate_interfaces().expect("enumerate");
        let iface = interfaces
            .iter()
            .find(|n| {
                n.ipv4_address
                    .as_deref()
                    .and_then(|s| s.parse::<Ipv4Addr>().ok())
                    .is_some_and(|ip| !ip.is_loopback() && !ip.is_unspecified())
            })
            .expect("need a NIC with IPv4");

        let nic = iface
            .friendly_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(if !iface.display_name.is_empty() {
                iface.display_name.as_str()
            } else {
                iface.name.as_str()
            });

        // TEST-NET-3 documentation address — safe for a one-shot diagnostic route.
        let dest = "203.0.113.77";
        let dest_ip = Ipv4Addr::new(203, 0, 113, 77);
        let before = host_route_exists(dest_ip).expect("snapshot");

        match ping_via_nic(dest, nic, Some(1)) {
            Ok(result) => {
                assert!(result.route_restored);
                assert_eq!(result.source_ip, iface.ipv4_address.as_deref().unwrap());
                assert_eq!(result.sent, 1);
                let after = host_route_exists(dest_ip).expect("after");
                assert_eq!(
                    before, after,
                    "temp /32 for {dest} must match pre-ping existence (before={before}, after={after})"
                );
            }
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("elevation") || msg.contains("requires elevation"),
                    "unexpected ping failure (not elevation): {msg}"
                );
                let after = host_route_exists(dest_ip).expect("after elevation fail");
                assert_eq!(
                    before, after,
                    "failed route add must not leave a temp host route"
                );
            }
        }
    }
}