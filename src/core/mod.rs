mod windivert_ffi;
use crate::config::{CompiledRule, Config};
use crate::network::enumerate_interfaces;
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

pub struct PacketRouter {
    config: Arc<Config>,
    running: Arc<AtomicBool>,
    compiled_rules: Vec<CompiledRule>,
}

impl PacketRouter {
    pub fn with_interfaces(config: Config) -> Result<Self> {
        let interfaces = enumerate_interfaces()?;
        let mut nic_index_map = HashMap::new();

        for nic in &interfaces {
            nic_index_map.insert(nic.name.to_ascii_lowercase(), nic.if_index);
            nic_index_map.insert(nic.display_name.to_ascii_lowercase(), nic.if_index);
        }

        let compiled_rules = config.compile_rules(&nic_index_map)?;

        Ok(PacketRouter {
            config: Arc::new(config),
            running: Arc::new(AtomicBool::new(false)),
            compiled_rules,
        })
    }

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

        for rule in &self.compiled_rules {
            log::info!(
                "  {} → {} (if_index={})",
                rule.ip_label,
                rule.nic,
                rule.if_index
            );
        }

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
                if let Some(rule) = Config::find_compiled(&self.compiled_rules, dst_ip) {
                    let mut net = addr.network();
                    net.if_idx = rule.if_index;
                    addr.set_network(net);
                    modified = true;
                    log::debug!(
                        "{} → NIC \"{}\" (if_idx={})",
                        dst_ip,
                        rule.nic,
                        rule.if_index
                    );
                    if let Some(new_ip) = rule.rewrite_to {
                        if let Some(ihl) = Self::ipv4_header_len(packet) {
                            let o = ihl - 4;
                            let octets = new_ip.octets();
                            packet[o] = octets[0];
                            packet[o + 1] = octets[1];
                            packet[o + 2] = octets[2];
                            packet[o + 3] = octets[3];
                            Self::recalc_ipv4_header_checksum(packet, ihl);
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
            if let Err(e) = handle.send(packet, &addr) {
                log::error!("WinDivertSend: {}", e);
            }
        }
        
        GLOBAL_HANDLE.store(std::ptr::null_mut(), Ordering::SeqCst);
        GLOBAL_RUNNING.store(false, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        println!(
            "[roust] Stopped. {} packets routed, {} passed through.",
            routed, passed
        );
        Ok(())
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
}
