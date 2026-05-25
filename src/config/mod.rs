use anyhow::{anyhow, Result};
use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};

/// Pre-parsed match pattern used on the packet hot path (no JSON or CIDR parsing per packet).
#[derive(Debug, Clone)]
pub enum IpMatch {
    Wildcard,
    Exact(Ipv4Addr),
    Network(IpNetwork),
}

/// One routing rule with NIC resolved to `if_index` at startup; kept entirely in memory while running.
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub ip_label: String,
    pub nic: String,
    pub match_pattern: IpMatch,
    pub if_index: u32,
    /// Primary IPv4 on the target NIC; used to rewrite outbound source when redirecting egress.
    pub egress_ipv4: Option<Ipv4Addr>,
    pub rewrite_to: Option<Ipv4Addr>,
}

impl CompiledRule {
    /// Return true when this rule's pattern matches the given IPv4 address.
    /// Outbound packets match on destination; inbound packets match on source (remote peer).
    pub fn matches(&self, dest: Ipv4Addr) -> bool {
        match &self.match_pattern {
            IpMatch::Wildcard => true,
            IpMatch::Exact(addr) => *addr == dest,
            IpMatch::Network(network) => network.contains(IpAddr::V4(dest)),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingRule {
    pub ip: String,
    pub nic: String,
    pub rewrite_to: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub rules: Vec<RoutingRule>,
}
impl Config {
    pub fn new() -> Self {
        Config { rules: vec![] }
    }
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(anyhow!("Config file not found: {:?}", path));
        }
        let contents = fs::read_to_string(path)?;
        Self::from_json_str(&contents)
    }
    pub fn from_json_str(contents: &str) -> Result<Self> {
        let rules: Vec<RoutingRule> = serde_json::from_str(contents).map_err(|e| {
            anyhow!("invalid routes JSON (expected [{{\"ip\":\"...\",\"nic\":\"...\"}}]): {e}")
        })?;
        Ok(Config { rules })
    }
    pub fn parse_import_file(contents: &str, path: &Path) -> Result<Vec<RoutingRule>> {
        let is_json = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
        if is_json {
            if let Ok(rules) = serde_json::from_str::<Vec<RoutingRule>>(contents) {
                return Ok(rules);
            }
            let ips: Vec<String> = serde_json::from_str(contents).map_err(|e| {                anyhow!(                    "JSON import must be [\"cidr\", ...] or [{{\"ip\":\"...\",\"nic\":\"...\"}}]: {e}"                )            })?;
            return Ok(ips
                .into_iter()
                .map(|ip| RoutingRule {
                    ip,
                    nic: String::new(),
                    rewrite_to: None,
                })
                .collect());
        }
        Ok(contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|ip| RoutingRule {
                ip: ip.to_string(),
                nic: String::new(),
                rewrite_to: None,
            })
            .collect())
    }
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.rules)?;
        fs::write(path, json)?;
        Ok(())
    }
    pub fn add_rule(&mut self, ip: String, nic: String, rewrite_to: Option<String>) -> Result<()> {
        if ip != "*" {
            self.validate_ip_format(&ip)?;
        }
        if let Some(ref rewrite) = rewrite_to {
            rewrite.parse::<IpAddr>()?;
        }
        let rule = RoutingRule {
            ip,
            nic,
            rewrite_to,
        };
        self.rules.push(rule);
        Ok(())
    }

    pub fn remove_rule(&mut self, ip: &str) -> bool {
        let initial_len = self.rules.len();
        self.rules.retain(|rule| rule.ip != ip);
        self.rules.len() < initial_len
    }

    fn validate_ip_format(&self, ip: &str) -> Result<()> {
        if ip.contains('/') {
            ip.parse::<IpNetwork>()?;
            return Ok(());
        }
        ip.parse::<IpAddr>()?;
        Ok(())
    }
    
    pub fn get_rules(&self) -> &[RoutingRule] {
        &self.rules
    }

    /// Build an in-memory routing table: parse each rule once and map NIC name → interface index.
    pub fn compile_rules(
        &self,
        nic_index_map: &HashMap<String, u32>,
        nic_ipv4_by_index: &HashMap<u32, Ipv4Addr>,
    ) -> Result<Vec<CompiledRule>> {
        let mut compiled = Vec::with_capacity(self.rules.len());
        for rule in &self.rules {
            let match_pattern = Self::compile_match_pattern(&rule.ip)?;
            let if_index = nic_index_map
                .get(&rule.nic.to_ascii_lowercase())
                .copied()
                .ok_or_else(|| {
                    anyhow!(
                        "routing rule {} → NIC \"{}\" has no matching interface on this machine",
                        rule.ip,
                        rule.nic
                    )
                })?;
            let rewrite_to = rule
                .rewrite_to
                .as_ref()
                .map(|s| {
                    s.parse::<Ipv4Addr>().map_err(|e| {
                        anyhow!("invalid rewrite_to \"{s}\" on rule {}: {e}", rule.ip)
                    })
                })
                .transpose()?;
            let egress_ipv4 = nic_ipv4_by_index.get(&if_index).copied();
            compiled.push(CompiledRule {
                ip_label: rule.ip.clone(),
                nic: rule.nic.clone(),
                match_pattern,
                if_index,
                egress_ipv4,
                rewrite_to,
            });
        }
        Ok(compiled)
    }

    /// Look up the first matching compiled rule for a destination IPv4 (in-memory only).
    pub fn find_compiled<'a>(
        compiled: &'a [CompiledRule],
        dest: Ipv4Addr,
    ) -> Option<&'a CompiledRule> {
        compiled.iter().find(|rule| rule.matches(dest))
    }

    fn compile_match_pattern(ip: &str) -> Result<IpMatch> {
        if ip == "*" {
            return Ok(IpMatch::Wildcard);
        }
        if let Ok(network) = ip.parse::<IpNetwork>() {
            return Ok(IpMatch::Network(network));
        }
        let addr: Ipv4Addr = ip
            .parse()
            .map_err(|e| anyhow!("invalid IPv4 or CIDR in routing rule \"{ip}\": {e}"))?;
        Ok(IpMatch::Exact(addr))
    }

    pub fn default_config_path() -> PathBuf {
        if let Ok(program_data) = std::env::var("ProgramData") {
            let path = PathBuf::from(program_data)
                .join("roust")
                .join("routes.json");
            if path.exists() {
                return path;
            }
        }
        let cwd_routes = PathBuf::from("routes.json");
        if cwd_routes.exists() {
            return cwd_routes;
        }
        PathBuf::from("routes.json")
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
    fn test_load_routes_json_array_format() {
        let json = r#"[            {"ip": "10.0.0.0/8", "nic": "Ethernet"},            {"ip": "192.168.0.0/16", "nic": "Wi-Fi"}        ]"#;
        let config = Config::from_json_str(json).unwrap();
        assert_eq!(config.rules.len(), 2);
        assert_eq!(config.rules[0].ip, "10.0.0.0/8");
        assert_eq!(config.rules[0].nic, "Ethernet");
    }
    #[test]
    fn test_parse_import_routes_objects() {
        let json = r#"[{"ip":"172.16.0.0/12","nic":"Ethernet"}]"#;
        let rules = Config::parse_import_file(json, Path::new("routes.json")).unwrap();
        assert_eq!(rules[0].nic, "Ethernet");
    }
    #[test]
    fn test_parse_import_cidr_string_array() {
        let json = r#"["10.0.0.0/8"]"#;
        let rules = Config::parse_import_file(json, Path::new("list.json")).unwrap();
        assert_eq!(rules[0].ip, "10.0.0.0/8");
        assert!(rules[0].nic.is_empty());
    }
    #[test]
    fn test_compile_rules_resolves_if_index() {
        let mut config = Config::new();
        config
            .add_rule("10.0.0.0/8".to_string(), "Ethernet".to_string(), None)
            .unwrap();
        let mut nic_map = HashMap::new();
        nic_map.insert("ethernet".to_string(), 42);
        let compiled = config.compile_rules(&nic_map, &HashMap::new()).unwrap();
        assert_eq!(compiled[0].if_index, 42);
        assert!(compiled[0].matches(Ipv4Addr::new(10, 1, 2, 3)));
    }

    #[test]
    fn test_find_compiled() {
        let mut config = Config::new();
        config
            .add_rule("192.168.1.0/24".to_string(), "Ethernet".to_string(), None)
            .unwrap();
        let mut nic_map = HashMap::new();
        nic_map.insert("ethernet".to_string(), 1);
        let compiled = config.compile_rules(&nic_map, &HashMap::new()).unwrap();
        let hit = Config::find_compiled(&compiled, Ipv4Addr::new(192, 168, 1, 50));
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().if_index, 1);
    }

}
