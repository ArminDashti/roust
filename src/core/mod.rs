mod windivert_ffi;

use crate::config::Config;
use crate::network::{enumerate_interfaces, predict_ipv4_egress, EgressPrediction};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;

use windivert_ffi::{
    safe::{self, WinDivertHandle},
    WinDivertAddress, WINDIVERT_LAYER_NETWORK, WINDIVERT_MTU_MAX, WINDIVERT_SHUTDOWN_RECV,
};

// ── Global state for the Ctrl-C handler ────────────────────────────────────
static GLOBAL_RUNNING: AtomicBool = AtomicBool::new(false);
static GLOBAL_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe extern "system" fn console_ctrl_handler(_ctrl_type: u32) -> i32 {
    GLOBAL_RUNNING.store(false, Ordering::SeqCst);
    let handle = GLOBAL_HANDLE.load(Ordering::SeqCst);
    if !handle.is_null() {
        windivert_ffi::WinDivertShutdown(handle, WINDIVERT_SHUTDOWN_RECV);
    }
    1 // TRUE – handled
}

pub struct PacketRouter {
    config: Arc<Config>,
    running: Arc<AtomicBool>,
    nic_index_map: HashMap<String, u32>,
}

impl PacketRouter {
    /// Create a router without resolving live NIC indices (useful for tests).
    pub fn new(config: Config) -> Self {
        PacketRouter {
            config: Arc::new(config),
            running: Arc::new(AtomicBool::new(false)),
            nic_index_map: HashMap::new(),
        }
    }

    /// Create a router and populate the NIC-name→if_index map from the system.
    pub fn with_interfaces(config: Config) -> Result<Self> {
        let interfaces = enumerate_interfaces()?;
        let mut nic_index_map = HashMap::new();
        for nic in &interfaces {
            nic_index_map.insert(nic.name.to_ascii_lowercase(), nic.if_index);
            nic_index_map.insert(nic.display_name.to_ascii_lowercase(), nic.if_index);
        }
        Ok(PacketRouter {
            config: Arc::new(config),
            running: Arc::new(AtomicBool::new(false)),
            nic_index_map,
        })
    }

    /// Run the packet capture/reroute loop (blocks until Ctrl-C or error).
    ///
    /// For every outbound IPv4 packet:
    ///   1. Capture it via WinDivert.
    ///   2. Match its destination against the routing rules.
    ///   3. If a rule matches, redirect the packet to the target NIC by
    ///      overwriting `WinDivertAddress.Network.IfIdx`.
    ///   4. Optionally rewrite the destination IP.
    ///   5. Recalculate checksums via `WinDivertHelperCalcChecksums`.
    ///   6. Re-inject the packet.
    pub fn run(&self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(anyhow!("Router is already running"));
        }

        let filter = "outbound and ip";
        let handle = WinDivertHandle::open(filter, WINDIVERT_LAYER_NETWORK, 0, 0)
            .map_err(|e| anyhow!("{}", e))?;

        log::info!(
            "WinDivert handle opened (filter=\"{}\"), {} routing rules loaded",
            filter,
            self.config.get_rules().len()
        );
        for rule in self.config.get_rules() {
            log::info!("  {} → {}", rule.ip, rule.nic);
        }

        // Publish state so the console Ctrl-C handler can trigger shutdown.
        self.running.store(true, Ordering::SeqCst);
        GLOBAL_RUNNING.store(true, Ordering::SeqCst);
        GLOBAL_HANDLE.store(handle.raw(), Ordering::SeqCst);

        unsafe {
            windivert_ffi::SetConsoleCtrlHandler(Some(console_ctrl_handler), 1);
        }

        println!("[roust] Packet router running. Press Ctrl+C to stop.");

        let mut buf = vec![0u8; WINDIVERT_MTU_MAX];
        let mut addr = WinDivertAddress::zeroed();
        let mut routed: u64 = 0;
        let mut passed: u64 = 0;

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
            let mut modified = false;

            if let Some(dst_ip) = Self::extract_dst_ipv4(packet) {
                let dst_str = dst_ip.to_string();

                if let Some((nic_name, rewrite_to)) = self.config.find_destination(&dst_str) {
                    // Redirect the packet to the matched NIC's interface index.
                    if let Some(&if_idx) =
                        self.nic_index_map.get(&nic_name.to_ascii_lowercase())
                    {
                        let mut net = addr.network();
                        net.if_idx = if_idx;
                        addr.set_network(net);
                        modified = true;

                        log::debug!("{} → NIC \"{}\" (if_idx={})", dst_str, nic_name, if_idx);
                    } else {
                        log::warn!(
                            "rule matched {} → NIC \"{}\" but that interface was not found",
                            dst_str,
                            nic_name
                        );
                    }

                    // Optionally rewrite the destination IP inside the packet.
                    if let Some(new_ip_str) = rewrite_to {
                        if let Ok(new_ip) = new_ip_str.parse::<Ipv4Addr>() {
                            let octets = new_ip.octets();
                            packet[16] = octets[0];
                            packet[17] = octets[1];
                            packet[18] = octets[2];
                            packet[19] = octets[3];
                            modified = true;
                        }
                    }

                    if modified {
                        let _ = safe::calc_checksums(packet, &mut addr);
                        routed += 1;
                    }
                }
            }

            if !modified {
                passed += 1;
            }

            // Re-inject every packet (modified or not) so traffic is never dropped.
            if let Err(e) = handle.send(packet, &addr) {
                log::error!("WinDivertSend: {}", e);
            }
        }

        // Cleanup: clear globals and unregister the handler.
        GLOBAL_HANDLE.store(std::ptr::null_mut(), Ordering::SeqCst);
        GLOBAL_RUNNING.store(false, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);

        println!(
            "[roust] Stopped. {} packets routed, {} passed through.",
            routed, passed
        );

        // `handle` drops here → WinDivertClose is called automatically.
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Which local NIC Windows would use for this IPv4 packet's destination,
    /// resolved via `GetBestRoute` (routing table).
    pub fn predict_egress_for_packet(&self, packet: &[u8]) -> Result<Option<EgressPrediction>> {
        let Some(dst) = Self::extract_dst_ipv4(packet) else {
            return Ok(None);
        };
        let prediction = predict_ipv4_egress(dst)?;
        Ok(Some(prediction))
    }

    fn extract_dst_ipv4(packet: &[u8]) -> Option<Ipv4Addr> {
        if packet.len() < 20 {
            return None;
        }
        Some(Ipv4Addr::new(
            packet[16],
            packet[17],
            packet[18],
            packet[19],
        ))
    }

    fn extract_dst_ip(packet: &[u8]) -> Option<String> {
        Self::extract_dst_ipv4(packet).map(|ip| ip.to_string())
    }

    fn apply_routing_rule(&self, packet: &mut [u8]) -> Option<String> {
        if packet.len() < 20 {
            return None;
        }

        let dst_ip = Self::extract_dst_ip(packet)?;
        let (nic, rewrite_to) = self.config.find_destination(&dst_ip)?;

        if let Some(new_ip) = rewrite_to {
            if let Ok(ip) = new_ip.parse::<Ipv4Addr>() {
                let octets = ip.octets();
                packet[16] = octets[0];
                packet[17] = octets[1];
                packet[18] = octets[2];
                packet[19] = octets[3];

                packet[10] = 0;
                packet[11] = 0;
                let mut sum: u32 = 0;
                for i in (0..20).step_by(2) {
                    sum += ((packet[i] as u32) << 8) | (packet[i + 1] as u32);
                }
                while (sum >> 16) != 0 {
                    sum = (sum & 0xFFFF) + (sum >> 16);
                }
                let checksum = !(sum as u16);
                packet[10] = (checksum >> 8) as u8;
                packet[11] = (checksum & 0xFF) as u8;
            }
        }

        Some(nic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_extract_dst_ip() {
        let mut packet = vec![0u8; 20];
        packet[16] = 192;
        packet[17] = 168;
        packet[18] = 1;
        packet[19] = 100;

        let dst = PacketRouter::extract_dst_ip(&packet);
        assert_eq!(dst, Some("192.168.1.100".to_string()));
    }

    #[test]
    fn test_apply_routing_with_rewrite() {
        let mut config = Config::new();
        config
            .add_rule(
                "192.168.1.0/24".to_string(),
                "Ethernet".to_string(),
                Some("10.0.0.1".to_string()),
            )
            .unwrap();

        let router = PacketRouter::new(config);

        let mut packet = vec![0u8; 20];
        packet[16] = 192;
        packet[17] = 168;
        packet[18] = 1;
        packet[19] = 100;

        let nic = router.apply_routing_rule(&mut packet);
        assert_eq!(nic, Some("Ethernet".to_string()));

        let new_dst = PacketRouter::extract_dst_ip(&packet);
        assert_eq!(new_dst, Some("10.0.0.1".to_string()));
    }

    #[test]
    fn test_predict_egress_short_packet() {
        let router = PacketRouter::new(Config::new());
        let r = router.predict_egress_for_packet(&[0u8; 4]).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn test_predict_egress_for_public_dns() {
        let router = PacketRouter::new(Config::new());
        let mut packet = vec![0u8; 20];
        packet[16] = 8;
        packet[17] = 8;
        packet[18] = 8;
        packet[19] = 8;
        let pred = router
            .predict_egress_for_packet(&packet)
            .expect("predict")
            .expect("some prediction");
        assert_eq!(pred.dest, Ipv4Addr::new(8, 8, 8, 8));
        assert!(pred.if_index > 0);
    }
}
