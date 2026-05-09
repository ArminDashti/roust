use anyhow::{anyhow, Result};
use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

/// A single routing rule
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingRule {
    /// IP address or CIDR range (e.g., "192.168.1.100" or "192.168.1.0/24" or "*")
    pub ip: String,
    /// Network interface name (NIC) destination
    pub nic: String,
    /// Optional IP address to rewrite the destination to
    pub rewrite_to: Option<String>,
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub rules: Vec<RoutingRule>,
}

impl Config {
    #[allow(dead_code)]
    /// Create a new empty configuration
    pub fn new() -> Self {
        Config { rules: vec![] }
    }

    /// Load configuration from a JSON file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(anyhow!("Config file not found: {:?}", path));
        }

        let contents = fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to a JSON file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Add a routing rule
    pub fn add_rule(&mut self, ip: String, nic: String, rewrite_to: Option<String>) -> Result<()> {
        // Validate IP format
        if ip != "*" {
            self.validate_ip_format(&ip)?;
        }
        if let Some(ref rewrite) = rewrite_to {
            rewrite.parse::<IpAddr>()?;
        }

        let rule = RoutingRule { ip, nic, rewrite_to };
        self.rules.push(rule);
        Ok(())
    }

    /// Remove a routing rule by IP
    pub fn remove_rule(&mut self, ip: &str) -> bool {
        let initial_len = self.rules.len();
        self.rules.retain(|rule| rule.ip != ip);
        self.rules.len() < initial_len
    }

    /// Find the destination NIC and optional rewrite address for a given IP address
    pub fn find_destination(&self, ip: &str) -> Option<(String, Option<String>)> {
        for rule in &self.rules {
            if self.ip_matches(&rule.ip, ip) {
                return Some((rule.nic.clone(), rule.rewrite_to.clone()));
            }
        }
        None
    }

    /// Check if an IP matches a rule pattern
    fn ip_matches(&self, pattern: &str, ip: &str) -> bool {
        // Wildcard matches everything
        if pattern == "*" {
            return true;
        }

        // Try to parse pattern as CIDR
        if let Ok(network) = pattern.parse::<IpNetwork>() {
            if let Ok(addr) = ip.parse::<IpAddr>() {
                return network.contains(addr);
            }
            return false;
        }

        // Try exact match
        pattern == ip
    }

    /// Validate IP format (CIDR or single IP)
    fn validate_ip_format(&self, ip: &str) -> Result<()> {
        if ip.contains('/') {
            ip.parse::<IpNetwork>()?;
            return Ok(());
        }

        // Try parsing as single IP
        ip.parse::<IpAddr>()?;
        Ok(())
    }

    /// Get all rules
    pub fn get_rules(&self) -> &[RoutingRule] {
        &self.rules
    }

    /// Clear all rules
    #[allow(dead_code)]
    pub fn clear_rules(&mut self) {
        self.rules.clear();
    }

    /// Get config path - tries common locations
    pub fn default_config_path() -> PathBuf {
        // Try %ProgramData%\roust\config.json first
        let program_data = std::env::var("ProgramData")
            .ok()
            .map(PathBuf::from)
            .map(|p| p.join("roust").join("config.json"));

        if let Some(path) = program_data {
            if path.exists() {
                return path;
            }
        }

        // Fall back to ./roust.json
        PathBuf::from("roust.json")
    }
}

impl Default for Config {
    fn default() -> Self {
        Config::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_matches_exact() {
        let config = Config::new();
        assert!(config.ip_matches("192.168.1.100", "192.168.1.100"));
        assert!(!config.ip_matches("192.168.1.100", "192.168.1.101"));
    }

    #[test]
    fn test_ip_matches_cidr() {
        let config = Config::new();
        assert!(config.ip_matches("192.168.1.0/24", "192.168.1.100"));
        assert!(config.ip_matches("192.168.1.0/24", "192.168.1.1"));
        assert!(!config.ip_matches("192.168.1.0/24", "192.168.2.1"));
    }

    #[test]
    fn test_ip_matches_wildcard() {
        let config = Config::new();
        assert!(config.ip_matches("*", "192.168.1.100"));
        assert!(config.ip_matches("*", "10.0.0.1"));
    }

    #[test]
    fn test_find_destination() {
        let mut config = Config::new();
        config.add_rule("192.168.1.0/24".to_string(), "Ethernet".to_string()).unwrap();
        config.add_rule("10.0.0.0/8".to_string(), "WiFi".to_string()).unwrap();
        config.add_rule("*".to_string(), "Ethernet".to_string()).unwrap();

        assert_eq!(config.find_destination("192.168.1.100"), Some("Ethernet".to_string()));
        assert_eq!(config.find_destination("10.5.5.5"), Some("WiFi".to_string()));
        assert_eq!(config.find_destination("172.16.0.1"), Some("Ethernet".to_string()));
    }
}
