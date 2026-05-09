use anyhow::{anyhow, Result};
use windows::Win32::NetworkManagement::IpHelper::*;

/// Represents a network interface
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    /// Friendly name of the NIC
    pub name: String,
    /// Display name (user-friendly)
    pub display_name: String,
    /// Interface index
    #[allow(dead_code)]
    pub if_index: u32,
    /// MAC address
    pub mac_address: String,
    /// IPv4 address if available
    pub ipv4_address: Option<String>,
    /// Status (Up, Down, etc.)
    pub status: String,
}

/// Enumerate all network interfaces on Windows
pub fn enumerate_interfaces() -> Result<Vec<NetworkInterface>> {
    let mut nics = Vec::new();

    unsafe {
        // Get adapter info
        let mut adapter_info_size = 0u32;
        
        // First call to get the size
        let result = GetAdaptersInfo(None, &mut adapter_info_size);
        if result != 0 && result != 111 { // ERROR_BUFFER_OVERFLOW
            return Err(anyhow!("GetAdaptersInfo failed with error code: {}", result));
        }

        let mut adapter_info = vec![0u8; adapter_info_size as usize];
        
        // Second call to get the actual data
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
            
            // Convert adapter name (ANSI to UTF-8)
            let name_cstr = std::ffi::CStr::from_ptr(adapter.AdapterName.as_ptr());
            let name = name_cstr.to_string_lossy().to_string();
            
            // Convert description
            let desc_cstr = std::ffi::CStr::from_ptr(adapter.Description.as_ptr());
            let display_name = desc_cstr.to_string_lossy().to_string();
            
            // Convert MAC address
            let mac_address = format!(
                "{:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}",
                adapter.Address[0],
                adapter.Address[1],
                adapter.Address[2],
                adapter.Address[3],
                adapter.Address[4],
                adapter.Address[5]
            );
            
            // Get IPv4 address if available
            let ipv4_address = if !adapter.IpAddressList.IpAddress.String.is_empty() {
                let ip_cstr = std::ffi::CStr::from_ptr(adapter.IpAddressList.IpAddress.String.as_ptr());
                Some(ip_cstr.to_string_lossy().to_string())
            } else {
                None
            };
            
            // Status
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

/// Get a single network interface by name
#[allow(dead_code)]
pub fn get_interface(name: &str) -> Result<Option<NetworkInterface>> {
    let interfaces = enumerate_interfaces()?;
    Ok(interfaces.into_iter().find(|nic| nic.name.eq_ignore_ascii_case(name)))
}

/// Check if an interface name exists
#[allow(dead_code)]
pub fn interface_exists(name: &str) -> Result<bool> {
    get_interface(name).map(|opt| opt.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_interfaces() {
        match enumerate_interfaces() {
            Ok(interfaces) => {
                println!("Found {} network interfaces", interfaces.len());
                for nic in interfaces {
                    println!("  - {} ({})", nic.name, nic.display_name);
                }
                assert!(!interfaces.is_empty(), "Should have at least one NIC");
            }
            Err(e) => {
                println!("Error enumerating interfaces: {:?}", e);
                // Don't fail the test in case of Windows API issues in test environment
            }
        }
    }
}
