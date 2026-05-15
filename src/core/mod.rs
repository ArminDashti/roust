mod windivert_ffi;

use crate::config::Config;
use crate::network::{predict_ipv4_egress, EgressPrediction};
use anyhow::{anyhow, Result};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct PacketRouter {
    config: Arc<Config>,
    running: Arc<AtomicBool>,
}

impl PacketRouter {
    pub fn new(config: Config) -> Self {
        PacketRouter {
            config: Arc::new(config),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(anyhow!("Router is already running"));
        }

        self.running.store(true, Ordering::SeqCst);
        println!("[INFO] Starting packet router...");

        // Create WinDivert handle for capturing outgoing packets
        let filter = "outbound and ip";
        match windivert_ffi::safe::WinDivert::new(filter, 0) {
            Ok(_divert) => {
                println!("[INFO] WinDivert handle created successfully");
                println!(
                    "[INFO] Loaded {} routing rules",
                    self.config.get_rules().len()
                );

                for rule in self.config.get_rules() {
                    println!("  - {} → {}", rule.ip, rule.nic);
                }

                Ok(())
            }
            Err(e) => {
                self.running.store(false, Ordering::SeqCst);
                Err(anyhow!("Failed to initialize WinDivert: {}", e))
            }
        }
    }

    pub fn stop(&mut self) -> Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(anyhow!("Router is not running"));
        }

        self.running.store(false, Ordering::SeqCst);
        println!("[INFO] Stopping packet router...");
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Which local NIC Windows would use for this IPv4 packet's destination, resolved via
    /// `GetBestRoute` (routing table) before relying on WinDivert `if_idx` on a live packet.
    pub fn predict_egress_for_packet(&self, packet: &[u8]) -> Result<Option<EgressPrediction>> {
        let Some(dst) = Self::extract_dst_ipv4(packet) else {
            return Ok(None);
        };
        let prediction = predict_ipv4_egress(dst)?;
        Ok(Some(prediction))
    }

    /// Extract destination IPv4 from an IPv4 packet (header only).
    fn extract_dst_ipv4(packet: &[u8]) -> Option<Ipv4Addr> {
        if packet.len() < 20 {
            return None;
        }
        let dst_bytes = &packet[16..20];
        Some(Ipv4Addr::new(
            dst_bytes[0],
            dst_bytes[1],
            dst_bytes[2],
            dst_bytes[3],
        ))
    }

    /// Extract destination IP from IPv4 packet
    fn extract_dst_ip(packet: &[u8]) -> Option<String> {
        Self::extract_dst_ipv4(packet).map(|ip| ip.to_string())
    }

    /// Modify IPv4 packet destination based on routing rules
    fn apply_routing_rule(&self, packet: &mut [u8]) -> Option<String> {
        if packet.len() < 20 {
            return None;
        }

        // Get destination IP
        let dst_ip = Self::extract_dst_ip(packet)?;

        // Find routing destination
        let (nic, rewrite_to) = self.config.find_destination(&dst_ip)?;

        if let Some(new_ip) = rewrite_to {
            if let Ok(ip) = new_ip.parse::<Ipv4Addr>() {
                let octets = ip.octets();
                packet[16] = octets[0];
                packet[17] = octets[1];
                packet[18] = octets[2];
                packet[19] = octets[3];

                // Recalculate IPv4 checksum (basic Internet Checksum)
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
        // Sample IPv4 packet with destination 192.168.1.100
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
        let mut config = crate::config::Config::new();
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
