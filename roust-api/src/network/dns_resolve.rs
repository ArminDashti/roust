//! Hostname → IPv4 A-record resolution for route compile / refresh.

use anyhow::{anyhow, Result};
use std::net::{Ipv4Addr, ToSocketAddrs};

/// Resolve `host` to all unicast IPv4 addresses (A records via getaddrinfo).
pub fn resolve_hostname_ipv4s(host: &str) -> Result<Vec<Ipv4Addr>> {
    let trimmed = host.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("hostname must not be empty"));
    }
    if let Ok(ip) = trimmed.parse::<Ipv4Addr>() {
        if ip.is_unspecified() || ip.is_multicast() {
            return Err(anyhow!("address {ip} is not a unicast IPv4 target"));
        }
        return Ok(vec![ip]);
    }
    if trimmed.parse::<std::net::Ipv6Addr>().is_ok() {
        return Err(anyhow!("IPv6 hosts are not supported; use an IPv4 hostname"));
    }

    let addrs = format!("{trimmed}:0")
        .to_socket_addrs()
        .map_err(|e| anyhow!("failed to resolve host '{trimmed}': {e}"))?;

    let mut out = Vec::new();
    for addr in addrs {
        if let std::net::IpAddr::V4(v4) = addr.ip() {
            if !v4.is_unspecified() && !v4.is_multicast() && !out.contains(&v4) {
                out.push(v4);
            }
        }
    }
    if out.is_empty() {
        return Err(anyhow!(
            "host '{trimmed}' did not resolve to an IPv4 address"
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_literal_ipv4() {
        let ips = resolve_hostname_ipv4s("8.8.8.8").unwrap();
        assert_eq!(ips, vec![Ipv4Addr::new(8, 8, 8, 8)]);
    }

    #[test]
    fn reject_empty() {
        assert!(resolve_hostname_ipv4s("  ").is_err());
    }
}
