mod windivert_run;

use crate::config::Config;
use crate::network::{predict_ipv4_egress, EgressPrediction};
use anyhow::Result;
use std::net::Ipv4Addr;
use std::sync::Arc;

#[cfg_attr(not(windows), allow(dead_code))]
pub struct PacketRouter {
    config: Arc<Config>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl PacketRouter {
    pub fn new(config: Config) -> Self {
        PacketRouter {
            config: Arc::new(config),
        }
    }

    /// Block until `run` is cleared (e.g. Ctrl+C) or an I/O error occurs.
    /// On Windows, captures outbound IPv4 with WinDivert, applies `rewrite_to` rules, reinjects.
    pub fn run_intercept_loop(&self, run: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<()> {
        windivert_run::run_intercept_loop(self, run)
    }

    /// Convenience: run with an internal stop flag and Ctrl+C wired in `run_intercept_loop`.
    pub fn start_blocking(&self) -> Result<()> {
        let run = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        self.run_intercept_loop(run)
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

    /// Extract destination IPv4 from an IPv4 packet (header only; respects IHL).
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

    /// Extract destination IP string for rule matching.
    fn extract_dst_ip(packet: &[u8]) -> Option<String> {
        Self::extract_dst_ipv4(packet).map(|ip| ip.to_string())
    }

    fn recalc_ipv4_header_checksum(packet: &mut [u8], header_len: usize) {
        if header_len < 20 || packet.len() < header_len {
            return;
        }
        packet[10] = 0;
        packet[11] = 0;
        let mut sum: u32 = 0;
        for i in (0..header_len).step_by(2) {
            sum += ((packet[i] as u32) << 8) | (packet[i + 1] as u32);
        }
        while (sum >> 16) != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }
        let checksum = !(sum as u16);
        packet[10] = (checksum >> 8) as u8;
        packet[11] = (checksum & 0xFF) as u8;
    }

    /// If a rule matches and defines `rewrite_to`, rewrite the IPv4 destination and fix the
    /// IPv4 header checksum. Returns `true` if the L4 checksums should be recomputed
    /// (caller should use WinDivertHelperCalcChecksums on Windows).
    pub fn apply_routing_rule(&self, packet: &mut [u8]) -> bool {
        let ihl = match Self::ipv4_header_len(packet) {
            Some(i) => i,
            None => return false,
        };
        let Some(dst_ip) = Self::extract_dst_ip(packet) else {
            return false;
        };
        let Some((_nic, rewrite_to)) = self.config.find_destination(&dst_ip) else {
            return false;
        };
        let Some(new_ip_str) = rewrite_to else {
            return false;
        };
        let Ok(new_ip) = new_ip_str.parse::<Ipv4Addr>() else {
            return false;
        };
        let o = ihl - 4;
        let octets = new_ip.octets();
        packet[o] = octets[0];
        packet[o + 1] = octets[1];
        packet[o + 2] = octets[2];
        packet[o + 3] = octets[3];
        Self::recalc_ipv4_header_checksum(packet, ihl);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_extract_dst_ip() {
        let mut packet = vec![0u8; 20];
        packet[0] = 0x45;
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
        packet[0] = 0x45;
        packet[16] = 192;
        packet[17] = 168;
        packet[18] = 1;
        packet[19] = 100;

        assert!(router.apply_routing_rule(&mut packet));

        let new_dst = PacketRouter::extract_dst_ip(&packet);
        assert_eq!(new_dst, Some("10.0.0.1".to_string()));
    }

    #[test]
    fn test_apply_routing_no_rewrite_leaves_dest() {
        let mut config = Config::new();
        config
            .add_rule("192.168.2.0/24".to_string(), "Ethernet".to_string(), None)
            .unwrap();
        let router = PacketRouter::new(config);
        let mut packet = vec![0u8; 20];
        packet[0] = 0x45;
        packet[16] = 192;
        packet[17] = 168;
        packet[18] = 2;
        packet[19] = 50;
        assert!(!router.apply_routing_rule(&mut packet));
        assert_eq!(
            PacketRouter::extract_dst_ip(&packet),
            Some("192.168.2.50".to_string())
        );
    }

    #[test]
    fn test_predict_egress_short_packet() {
        let router = PacketRouter::new(Config::new());
        let r = router.predict_egress_for_packet(&[0u8; 4]).unwrap();
        assert!(r.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn test_predict_egress_for_public_dns() {
        let router = PacketRouter::new(Config::new());
        let mut packet = vec![0u8; 20];
        packet[0] = 0x45;
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
