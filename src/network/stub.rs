use anyhow::{anyhow, Result};
use std::net::Ipv4Addr;

use super::EgressPrediction;
use super::NetworkInterface;

pub fn enumerate_interfaces() -> Result<Vec<NetworkInterface>> {
    Err(anyhow!(
        "Network interface enumeration is only supported on Windows targets"
    ))
}

/// Stub: routing table lookup requires Windows `iphlpapi`.
pub fn predict_ipv4_egress(_dest: Ipv4Addr) -> Result<EgressPrediction> {
    Err(anyhow!(
        "Egress NIC prediction requires a Windows build (GetBestRoute)"
    ))
}
