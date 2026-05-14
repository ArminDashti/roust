use anyhow::{anyhow, Result};
use std::net::Ipv4Addr;
use windows::Win32::NetworkManagement::IpHelper::*;

use super::EgressPrediction;
use super::NetworkInterface;

pub fn enumerate_interfaces() -> Result<Vec<NetworkInterface>> {
    let mut nics = Vec::new();

    unsafe {
        let mut adapter_info_size = 0u32;

        let result = GetAdaptersInfo(None, &mut adapter_info_size);
        if result != 0 && result != 111 {
            return Err(anyhow!("GetAdaptersInfo failed with error code: {}", result));
        }

        let mut adapter_info = vec![0u8; adapter_info_size as usize];

        let result = GetAdaptersInfo(
            Some(adapter_info.as_mut_ptr() as *mut _),
            &mut adapter_info_size,
        );
        if result != 0 {
            return Err(anyhow!("GetAdaptersInfo failed with error code: {}", result));
        }

        let mut current = adapter_info.as_ptr() as *mut IP_ADAPTER_INFO;

        while !current.is_null() {
            let adapter = &*current;

            let name_cstr = std::ffi::CStr::from_ptr(adapter.AdapterName.as_ptr());
            let name = name_cstr.to_string_lossy().to_string();

            let desc_cstr = std::ffi::CStr::from_ptr(adapter.Description.as_ptr());
            let display_name = desc_cstr.to_string_lossy().to_string();

            let mac_address = format!(
                "{:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}",
                adapter.Address[0],
                adapter.Address[1],
                adapter.Address[2],
                adapter.Address[3],
                adapter.Address[4],
                adapter.Address[5]
            );

            let ipv4_address = if !adapter.IpAddressList.IpAddress.String.is_empty() {
                let ip_cstr = std::ffi::CStr::from_ptr(adapter.IpAddressList.IpAddress.String.as_ptr());
                Some(ip_cstr.to_string_lossy().to_string())
            } else {
                None
            };

            let status = match adapter.Type {
                6 => "Ethernet".to_string(),
                71 => "WiFi".to_string(),
                1 => "Other".to_string(),
                _ => format!("Type({})", adapter.Type),
            };

            nics.push(NetworkInterface {
                name,
                display_name,
                if_index: adapter.Index,
                mac_address,
                ipv4_address,
                status,
            });

            current = adapter.Next;
        }
    }

    Ok(nics)
}

pub fn get_interface(name: &str) -> Result<Option<NetworkInterface>> {
    let interfaces = enumerate_interfaces()?;
    Ok(interfaces
        .into_iter()
        .find(|nic| nic.name.eq_ignore_ascii_case(name)))
}

pub fn interface_exists(name: &str) -> Result<bool> {
    get_interface(name).map(|opt| opt.is_some())
}

/// Uses the same routing-table resolution the TCP/IP stack will use for the first
/// outbound packet to `dest` (see `GetBestRoute`). This runs in user space before any
/// WinDivert `if_idx` is observed on a captured packet.
pub fn predict_ipv4_egress(dest: Ipv4Addr) -> Result<EgressPrediction> {
    let dest_arg = dest.to_bits();
    let mut row = MIB_IPFORWARDROW::default();

    let rc = unsafe { GetBestRoute(dest_arg, 0, &mut row) };
    if rc != 0 {
        return Err(anyhow!(
            "GetBestRoute failed for {}: Win32 error {:?}",
            dest,
            rc
        ));
    }

    let if_index = row.dwForwardIfIndex;
    let next_hop = Ipv4Addr::from_bits(row.dwForwardNextHop);

    let interfaces = enumerate_interfaces()?;
    let nic = interfaces
        .iter()
        .find(|n| n.if_index == if_index)
        .cloned();

    let nic_name = nic.as_ref().map(|n| n.name.clone());
    let nic_display = nic.as_ref().map(|n| n.display_name.clone());

    Ok(EgressPrediction {
        dest,
        if_index,
        next_hop,
        nic_name,
        nic_display,
    })
}
