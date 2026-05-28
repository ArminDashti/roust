use super::EgressPrediction;
use super::NetworkInterface;
use anyhow::{anyhow, Result};
use std::net::Ipv4Addr;
use windows::Win32::NetworkManagement::IpHelper::*;
use windows::Win32::Networking::WinSock::{AF_INET, AF_UNSPEC, SOCKADDR_IN};

pub fn nic_name_matches(nic: &NetworkInterface, nic_name: &str) -> bool {
    [
        Some(nic.name.as_str()),
        Some(nic.display_name.as_str()),
        nic.friendly_name.as_deref(),
    ]
        .into_iter()
        .flatten()
        .any(|label| label.eq_ignore_ascii_case(nic_name))
}

fn if_type_label(if_type: u32) -> String {
    match if_type {
        6 => "Ethernet".to_string(),
        71 => "WiFi".to_string(),
        1 => "Other".to_string(),
        _ => format!("Type({if_type})"),
    }
}

fn ipv4_from_sockaddr(sa: *mut windows::Win32::Networking::WinSock::SOCKADDR) -> Option<Ipv4Addr> {
    unsafe {
        if sa.is_null() || (*sa).sa_family != AF_INET {
            return None;
        }
        let sin = &*(sa as *const SOCKADDR_IN);
        let raw = sin.sin_addr.S_un.S_addr;
        Some(Ipv4Addr::from_bits(u32::from_be(raw)))
    }
}

fn first_ipv4_from_adapter(adapter: &IP_ADAPTER_ADDRESSES_LH) -> Option<String> {
    unsafe {
        let mut unicast = adapter.FirstUnicastAddress;
        while !unicast.is_null() {
            let row = &*unicast;
            if let Some(addr) = ipv4_from_sockaddr(row.Address.lpSockaddr) {
                return Some(addr.to_string());
            }
            unicast = row.Next;
        }
    }
    None
}

fn first_gateway_from_adapter(adapter: &IP_ADAPTER_ADDRESSES_LH) -> Option<Ipv4Addr> {
    unsafe {
        let mut gateway = adapter.FirstGatewayAddress;
        while !gateway.is_null() {
            let row = &*gateway;
            if let Some(addr) = ipv4_from_sockaddr(row.Address.lpSockaddr) {
                return Some(addr);
            }
            gateway = row.Next;
        }
    }
    None
}

pub fn enumerate_interfaces() -> Result<Vec<NetworkInterface>> {
    let mut nics = Vec::new();
    let mut size = 0u32;
    let flags = GAA_FLAG_INCLUDE_PREFIX | GAA_FLAG_INCLUDE_GATEWAYS;

    unsafe {
        let _ = GetAdaptersAddresses(AF_UNSPEC.0.into(), flags, None, None, &mut size);
        let mut buffer = vec![0u8; size as usize];
        let ptr = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        let rc = GetAdaptersAddresses(AF_UNSPEC.0.into(), flags, None, Some(ptr), &mut size);
        if rc != 0 {
            return Err(anyhow!(
                "GetAdaptersAddresses failed with error code: {}",
                rc
            ));
        }

        let mut current = ptr;
        while !current.is_null() {
            let adapter = &*current;
            let if_index = adapter.Anonymous1.Anonymous.IfIndex;

            let name = if adapter.AdapterName.0.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(adapter.AdapterName.as_ptr().cast())
                    .to_string_lossy()
                    .into_owned()
            };

            let display_name = if adapter.Description.is_null() {
                String::new()
            } else {
                adapter.Description.to_string()?
            };

            let friendly_name = if adapter.FriendlyName.0.is_null() {
                None
            } else {
                let friendly = adapter.FriendlyName.to_string()?;
                if friendly.is_empty() {
                    None
                } else {
                    Some(friendly)
                }
            };

            let mac_address = if adapter.PhysicalAddressLength >= 6 {
                format!(
                    "{:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}",
                    adapter.PhysicalAddress[0],
                    adapter.PhysicalAddress[1],
                    adapter.PhysicalAddress[2],
                    adapter.PhysicalAddress[3],
                    adapter.PhysicalAddress[4],
                    adapter.PhysicalAddress[5]
                )
            } else {
                "N/A".to_string()
            };

            let ipv4_address = first_ipv4_from_adapter(adapter);
            let default_gateway = first_gateway_from_adapter(adapter);
            let status = if_type_label(adapter.IfType);

            nics.push(NetworkInterface {
                name,
                display_name,
                friendly_name,
                default_gateway,
                if_index,
                mac_address,
                ipv4_address,
                status,
            });

            current = adapter.Next;
        }
    }

    Ok(nics)
}

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
    let nic = interfaces.iter().find(|n| n.if_index == if_index).cloned();
    let nic_name = nic.as_ref().map(|n| n.name.clone());
    let nic_display = nic.as_ref().map(|n| n.display_name.clone());
    let nic_friendly = nic.as_ref().map(|n| n.friendly_name.clone());

    Ok(EgressPrediction {
        dest,
        if_index,
        next_hop,
        nic_name,
        nic_display,
        nic_friendly,
    })
}
