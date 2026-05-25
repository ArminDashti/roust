mod windivert_ffi;
use crate::config::{CompiledRule, Config};
use crate::network::{enumerate_interfaces, install_routes_for_rules, remove_installed_routes, InstalledRoute};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;
use windivert_ffi::{
    safe::{self, WinDivertHandle},
    WinDivertAddress, WINDIVERT_LAYER_NETWORK, WINDIVERT_MTU_MAX, WINDIVERT_SHUTDOWN_RECV,
};

static GLOBAL_RUNNING: AtomicBool = AtomicBool::new(false);
static GLOBAL_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe extern "system" fn console_ctrl_handler(_ctrl_type: u32) -> i32 {
    GLOBAL_RUNNING.store(false, Ordering::SeqCst);
    let handle = GLOBAL_HANDLE.load(Ordering::SeqCst);
    if !handle.is_null() {
        windivert_ffi::WinDivertShutdown(handle, WINDIVERT_SHUTDOWN_RECV);
    }
    1
}

#[derive(Debug, Default)]
struct RouteStats {
    outbound_routed: u64,
    outbound_passed: u64,
    inbound_routed: u64,
    inbound_passed: u64,
}

pub struct PacketRouter {
    config: Arc<Config>,
    running: Arc<AtomicBool>,
    compiled_rules: Vec<CompiledRule>,
    installed_routes: Vec<InstalledRoute>,
}

impl PacketRouter {
    pub fn with_interfaces(config: Config) -> Result<Self> {
        let interfaces = enumerate_interfaces()?;
        let mut nic_index_map = HashMap::new();

        let mut nic_ipv4_by_index = HashMap::new();
        let mut nic_gateway_by_index = HashMap::new();

        for nic in &interfaces {
            nic_index_map.insert(nic.name.to_ascii_lowercase(), nic.if_index);
            nic_index_map.insert(nic.display_name.to_ascii_lowercase(), nic.if_index);
            if let Some(alias) = &nic.friendly_name {
                nic_index_map.insert(alias.to_ascii_lowercase(), nic.if_index);
            }
            if let Some(ip) = &nic.ipv4_address {
                if let Ok(addr) = ip.parse::<Ipv4Addr>() {
                    if !addr.is_unspecified() && !addr.is_loopback() {
                        nic_ipv4_by_index.insert(nic.if_index, addr);
                    }
                }
            }
            if let Some(gw) = nic.default_gateway {
                if !gw.is_unspecified() {
                    nic_gateway_by_index.insert(nic.if_index, gw);
                }
            }
        }

        let compiled_rules = config.compile_rules(&nic_index_map, &nic_ipv4_by_index)?;
        let installed_routes = install_routes_for_rules(&compiled_rules, &nic_gateway_by_index)?;

        Ok(PacketRouter {
            config: Arc::new(config),
            running: Arc::new(AtomicBool::new(false)),
            compiled_rules,
            installed_routes,
        })
    }

    pub fn run(&self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(anyhow!("Router is already running"));
        }

        let filter = "ip";
        let handle = WinDivertHandle::open(filter, WINDIVERT_LAYER_NETWORK, 0, 0)
            .map_err(|e| anyhow!("{}", e))?;
        log::info!(
            "WinDivert handle opened (filter=\"{}\"), {} routing rules loaded",
            filter,
            self.config.get_rules().len()
        );

        for rule in &self.compiled_rules {
            match rule.egress_ipv4 {
                Some(egress) => log::info!(
                    "  {} → {} (if_index={}, egress_src={})",
                    rule.ip_label,
                    rule.nic,
                    rule.if_index,
                    egress
                ),
                None => log::info!(
                    "  {} → {} (if_index={})",
                    rule.ip_label,
                    rule.nic,
                    rule.if_index
                ),
            }
        }

        self.running.store(true, Ordering::SeqCst);
        GLOBAL_RUNNING.store(true, Ordering::SeqCst);
        GLOBAL_HANDLE.store(handle.raw(), Ordering::SeqCst);

        unsafe {
            windivert_ffi::SetConsoleCtrlHandler(Some(console_ctrl_handler), 1);
        }

        println!("[roust] Packet router running (inbound + outbound IPv4). Press Ctrl+C to stop.");

        let mut buf = vec![0u8; WINDIVERT_MTU_MAX];
        let mut addr = WinDivertAddress::zeroed();
        let mut stats = RouteStats::default();

        loop {
            if !GLOBAL_RUNNING.load(Ordering::SeqCst) {
                break;
            }

            let recv_len = match handle.recv(&mut buf, &mut addr) {
                Ok(n) => n as usize,
                Err(e) => {
                    if !GLOBAL_RUNNING.load(Ordering::SeqCst) {
                        break;
                    }
                    log::error!("WinDivertRecv: {}", e);
                    continue;
                }
            };

            let packet = &mut buf[..recv_len];
            let outbound = addr.is_outbound();
            let routed = Self::apply_rules(packet, &mut addr, &self.compiled_rules, outbound);
            if routed {
                if outbound {
                    stats.outbound_routed += 1;
                } else {
                    stats.inbound_routed += 1;
                }
            } else if outbound {
                stats.outbound_passed += 1;
            } else {
                stats.inbound_passed += 1;
            }

            if let Err(e) = handle.send(packet, &addr) {
                log::error!("WinDivertSend: {}", e);
            }
        }

        GLOBAL_HANDLE.store(std::ptr::null_mut(), Ordering::SeqCst);
        GLOBAL_RUNNING.store(false, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        remove_installed_routes(&self.installed_routes);
        println!(
            "[roust] Stopped. outbound: {} routed, {} passed; inbound: {} routed, {} passed.",
            stats.outbound_routed,
            stats.outbound_passed,
            stats.inbound_routed,
            stats.inbound_passed
        );
        Ok(())
    }

    /// Apply routing rules to one IPv4 packet. Returns true when a rule matched and changed metadata/header.
    fn apply_rules(
        packet: &mut [u8],
        addr: &mut WinDivertAddress,
        compiled_rules: &[CompiledRule],
        outbound: bool,
    ) -> bool {
        let match_ip = if outbound {
            Self::extract_dst_ipv4(packet)
        } else {
            Self::extract_src_ipv4(packet)
        };
        let Some(match_ip) = match_ip else {
            return false;
        };

        let Some(rule) = Config::find_compiled(compiled_rules, match_ip) else {
            return false;
        };

        // NIC selection is handled via kernel routes installed at startup.
        // WinDivert ignores outbound IfIdx; only apply packet edits for rewrite_to.
        let Some(new_ip) = rule.rewrite_to else {
            return false;
        };

        let direction = if outbound { "outbound" } else { "inbound" };
        log::debug!(
            "{direction} {} rewrite on NIC \"{}\" (if_idx={})",
            match_ip,
            rule.nic,
            rule.if_index
        );

        let header_changed = if outbound {
            Self::rewrite_dst_ipv4(packet, new_ip)
        } else {
            Self::rewrite_src_ipv4(packet, new_ip)
        };

        if header_changed {
            addr.invalidate_checksum_flags();
            if let Err(e) = safe::calc_checksums(packet, addr) {
                log::warn!("WinDivertHelperCalcChecksums failed: {e}");
            }
        }

        true
    }

    fn ipv4_header_len(packet: &[u8]) -> Option<usize> {
        let first = *packet.first()?;
        if (first >> 4) != 4 {
            return None;
        }
        let ihl = ((first & 0x0f) as usize) * 4;
        if ihl < 20 || packet.len() < ihl {
            return None;
        }
        Some(ihl)
    }

    fn extract_dst_ipv4(packet: &[u8]) -> Option<Ipv4Addr> {
        let ihl = Self::ipv4_header_len(packet)?;
        let o = ihl - 4;
        Some(Ipv4Addr::new(
            packet[o],
            packet[o + 1],
            packet[o + 2],
            packet[o + 3],
        ))
    }

    fn extract_src_ipv4(packet: &[u8]) -> Option<Ipv4Addr> {
        Self::ipv4_header_len(packet)?;
        Some(Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15]))
    }

    fn rewrite_dst_ipv4(packet: &mut [u8], new_ip: Ipv4Addr) -> bool {
        let o = match Self::ipv4_header_len(packet) {
            Some(ihl) => ihl - 4,
            None => return false,
        };
        let octets = new_ip.octets();
        packet[o] = octets[0];
        packet[o + 1] = octets[1];
        packet[o + 2] = octets[2];
        packet[o + 3] = octets[3];
        packet[10] = 0;
        packet[11] = 0;
        true
    }

    fn rewrite_src_ipv4(packet: &mut [u8], new_ip: Ipv4Addr) -> bool {
        if Self::ipv4_header_len(packet).is_none() {
            return false;
        }
        let octets = new_ip.octets();
        packet[12] = octets[0];
        packet[13] = octets[1];
        packet[14] = octets[2];
        packet[15] = octets[3];
        packet[10] = 0;
        packet[11] = 0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ipv4_packet(src: [u8; 4], dst: [u8; 4]) -> Vec<u8> {
        vec![
            0x45, 0x00, 0x00, 0x1c, // ver/ihl, tos, total length
            0, 0, 0, 0, 0, 0, 0, 0, // id, flags, ttl, proto, checksum
            src[0], src[1], src[2], src[3], dst[0], dst[1], dst[2], dst[3],
        ]
    }

    #[test]
    fn extract_src_and_dst() {
        let pkt = sample_ipv4_packet([10, 0, 0, 1], [8, 8, 8, 8]);
        assert_eq!(
            PacketRouter::extract_src_ipv4(&pkt),
            Some(Ipv4Addr::new(10, 0, 0, 1))
        );
        assert_eq!(
            PacketRouter::extract_dst_ipv4(&pkt),
            Some(Ipv4Addr::new(8, 8, 8, 8))
        );
    }

    #[test]
    fn route_only_rule_does_not_edit_packets() {
        let mut pkt = sample_ipv4_packet([10, 138, 172, 26], [212, 80, 19, 12]);
        let mut addr = WinDivertAddress::zeroed();
        let rules = vec![CompiledRule {
            ip_label: "212.80.19.12".to_string(),
            nic: "Ethernet".to_string(),
            match_pattern: crate::config::IpMatch::Exact(Ipv4Addr::new(212, 80, 19, 12)),
            if_index: 21,
            egress_ipv4: Some(Ipv4Addr::new(192, 168, 1, 101)),
            rewrite_to: None,
        }];
        assert!(!PacketRouter::apply_rules(&mut pkt, &mut addr, &rules, true));
        assert_eq!(
            PacketRouter::extract_src_ipv4(&pkt),
            Some(Ipv4Addr::new(10, 138, 172, 26))
        );
    }

    #[test]
    fn rewrite_src_changes_address() {
        let mut pkt = sample_ipv4_packet([10, 0, 0, 1], [8, 8, 8, 8]);
        assert!(PacketRouter::rewrite_src_ipv4(
            &mut pkt,
            Ipv4Addr::new(192, 168, 1, 1)
        ));
        assert_eq!(
            PacketRouter::extract_src_ipv4(&pkt),
            Some(Ipv4Addr::new(192, 168, 1, 1))
        );
        assert_eq!(pkt[10], 0);
        assert_eq!(pkt[11], 0);
    }
}
