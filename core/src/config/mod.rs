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

/// One routing rule with gateway resolved to `if_index` at startup; kept entirely in memory while running.
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub ip_label: String,
    pub gateway: Ipv4Addr,
    pub match_pattern: IpMatch,
    pub if_index: u32,
    /// Primary IPv4 on the target interface; used to rewrite outbound source when redirecting egress.
    pub egress_ipv4: Option<Ipv4Addr>,
    pub rewrite_to: Option<Ipv4Addr>,
}

/// Resolved interface info built at startup from live adapter enumeration.
#[derive(Debug, Clone)]
pub struct MacEntry {
    pub if_index: u32,
    pub gateway: Ipv4Addr,
    pub egress_ipv4: Option<Ipv4Addr>,
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

/// A routing rule as stored in `routes.json`.
///
/// Resolution priority at startup: MAC > NIC name > gateway IP.
/// At least one target field must be set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingRule {
    #[serde(rename = "ip-or-cidr")]
    pub ip: String,
    #[serde(rename = "route-to-mac-address")]
    pub mac: Option<String>,
    #[serde(rename = "route-to-nic-name")]
    pub nic: Option<String>,
    #[serde(rename = "route-to-default-gateway")]
    pub gateway: Option<String>,
    pub rewrite_to: Option<String>,
}

/// Input struct for loading `routes.json`; catches deprecated field names with clear hints.
#[derive(Debug, Deserialize)]
struct RoutingRuleInput {
    #[serde(rename = "ip-or-cidr", alias = "ip")]
    ip: String,
    #[serde(rename = "route-to-mac-address", default)]
    mac: Option<String>,
    #[serde(rename = "route-to-nic-name", default)]
    nic: Option<String>,
    #[serde(rename = "route-to-default-gateway", default)]
    gateway: Option<String>,
    pub rewrite_to: Option<String>,
}

impl RoutingRuleInput {
    fn into_rule(self) -> Result<RoutingRule> {
        if self.mac.is_none() && self.nic.is_none() && self.gateway.is_none() {
            return Err(anyhow!(
                "routing rule {} must set at least one of \"route-to-mac-address\", \
                 \"route-to-nic-name\", or \"route-to-default-gateway\" \
                 (run `roust gateway list` to see available adapters).",
                self.ip
            ));
        }
        Ok(RoutingRule {
            ip: self.ip,
            mac: self.mac.filter(|m| !m.is_empty()).map(|m| m.to_ascii_uppercase()),
            nic: self.nic.filter(|n| !n.is_empty()),
            gateway: self.gateway.filter(|g| !g.is_empty()),
            rewrite_to: self.rewrite_to,
        })
    }
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
        let inputs: Vec<RoutingRuleInput> = serde_json::from_str(contents).map_err(|e| {
            anyhow!(
                "invalid routes JSON (expected \
                 [{{\"ip-or-cidr\":\"...\",\"route-to-mac-address\":\"...\"}}]): {e}"
            )
        })?;
        let rules = inputs
            .into_iter()
            .map(RoutingRuleInput::into_rule)
            .collect::<Result<Vec<_>>>()?;
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
            let ips: Vec<String> = serde_json::from_str(contents).map_err(|e| {
                anyhow!(
                    "JSON import must be [\"cidr\", ...] or \
                     [{{\"ip-or-cidr\":\"...\",\"route-to-mac-address\":\"...\"}}]: {e}"
                )
            })?;
            return Ok(ips
                .into_iter()
                .map(|ip| RoutingRule {
                    ip,
                    mac: None,
                    gateway: None,
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
                mac: None,
                gateway: None,
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

    pub fn add_rule(
        &mut self,
        ip: String,
        mac: Option<String>,
        nic: Option<String>,
        gateway: Option<String>,
        rewrite_to: Option<String>,
    ) -> Result<()> {
        if ip != "*" {
            self.validate_ip_format(&ip)?;
        }
        let mac = mac.filter(|m| !m.is_empty()).map(|m| m.to_ascii_uppercase());
        let nic = nic.filter(|n| !n.is_empty());
        let gateway = gateway.filter(|g| !g.is_empty());
        if mac.is_none() && nic.is_none() && gateway.is_none() {
            return Err(anyhow!(
                "provide at least one of --mac, --nic, or --gateway"
            ));
        }
        if let Some(ref gw) = gateway {
            gw.parse::<Ipv4Addr>()
                .map_err(|e| anyhow!("invalid gateway \"{gw}\": {e}"))?;
        }
        if let Some(ref rewrite) = rewrite_to {
            rewrite.parse::<IpAddr>()?;
        }
        self.rules.push(RoutingRule { ip, mac, nic, gateway, rewrite_to });
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

    /// Build an in-memory routing table.
    ///
    /// Resolution priority per rule: MAC > NIC name > gateway IP.
    pub fn compile_rules(
        &self,
        mac_map: &HashMap<String, MacEntry>,
        nic_map: &HashMap<String, MacEntry>,
        gw_map: &HashMap<Ipv4Addr, MacEntry>,
    ) -> Result<Vec<CompiledRule>> {
        let mut compiled = Vec::with_capacity(self.rules.len());
        for rule in &self.rules {
            let match_pattern = Self::compile_match_pattern(&rule.ip)?;
            let entry = Self::resolve_entry(rule, mac_map, nic_map, gw_map)?;
            let rewrite_to = rule
                .rewrite_to
                .as_ref()
                .map(|s| {
                    s.parse::<Ipv4Addr>()
                        .map_err(|e| anyhow!("invalid rewrite_to \"{s}\" on rule {}: {e}", rule.ip))
                })
                .transpose()?;
            compiled.push(CompiledRule {
                ip_label: rule.ip.clone(),
                gateway: entry.gateway,
                match_pattern,
                if_index: entry.if_index,
                egress_ipv4: entry.egress_ipv4,
                rewrite_to,
            });
        }
        Ok(compiled)
    }

    /// Priority: MAC > NIC name > gateway IP.
    fn resolve_entry<'a>(
        rule: &RoutingRule,
        mac_map: &'a HashMap<String, MacEntry>,
        nic_map: &'a HashMap<String, MacEntry>,
        gw_map: &'a HashMap<Ipv4Addr, MacEntry>,
    ) -> Result<&'a MacEntry> {
        if let Some(mac) = &rule.mac {
            return mac_map.get(&mac.to_ascii_uppercase()).ok_or_else(|| {
                anyhow!(
                    "routing rule {} → MAC {} not found on any local interface",
                    rule.ip, mac
                )
            });
        }
        if let Some(nic) = &rule.nic {
            return nic_map.get(&nic.to_ascii_lowercase()).ok_or_else(|| {
                anyhow!(
                    "routing rule {} → NIC name \"{}\" not found on any local interface",
                    rule.ip, nic
                )
            });
        }
        if let Some(gw_str) = &rule.gateway {
            let gw: Ipv4Addr = gw_str.parse().map_err(|e| {
                anyhow!("routing rule {} has invalid gateway \"{gw_str}\": {e}", rule.ip)
            })?;
            return gw_map.get(&gw).ok_or_else(|| {
                anyhow!(
                    "routing rule {} → gateway {} is not a default gateway on any local interface",
                    rule.ip, gw_str
                )
            });
        }
        Err(anyhow!(
            "routing rule {} has none of route-to-mac-address, route-to-nic-name, \
             or route-to-default-gateway",
            rule.ip
        ))
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
    fn test_reject_deprecated_nic_field() {
        let json = r#"[{"ip-or-cidr":"10.0.0.0/8","nic":"Ethernet"}]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("deprecated"));
    }

    #[test]
    fn test_reject_rule_with_neither_mac_nor_gateway() {
        let json = r#"[{"ip-or-cidr":"10.0.0.0/8"}]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("route-to-mac-address"));
        assert!(err.to_string().contains("route-to-default-gateway"));
    }

    #[test]
    fn test_load_mac_only_rule() {
        let json = r#"[
            {"ip-or-cidr": "10.0.0.0/8", "route-to-mac-address": "AA:BB:CC:DD:EE:FF"}
        ]"#;
        let config = Config::from_json_str(json).unwrap();
        assert_eq!(config.rules[0].mac.as_deref(), Some("AA:BB:CC:DD:EE:FF"));
        assert!(config.rules[0].nic.is_none());
        assert!(config.rules[0].gateway.is_none());
    }

    #[test]
    fn test_load_nic_only_rule() {
        let json = r#"[
            {"ip-or-cidr": "10.0.0.0/8", "route-to-nic-name": "Wi-Fi"}
        ]"#;
        let config = Config::from_json_str(json).unwrap();
        assert!(config.rules[0].mac.is_none());
        assert_eq!(config.rules[0].nic.as_deref(), Some("Wi-Fi"));
        assert!(config.rules[0].gateway.is_none());
    }

    #[test]
    fn test_load_gateway_only_rule() {
        let json = r#"[
            {"ip-or-cidr": "10.0.0.0/8", "route-to-default-gateway": "192.168.1.1"}
        ]"#;
        let config = Config::from_json_str(json).unwrap();
        assert!(config.rules[0].mac.is_none());
        assert!(config.rules[0].nic.is_none());
        assert_eq!(config.rules[0].gateway.as_deref(), Some("192.168.1.1"));
    }

    #[test]
    fn test_load_all_three_fields() {
        let json = r#"[
            {
                "ip-or-cidr": "10.0.0.0/8",
                "route-to-mac-address": "AA:BB:CC:DD:EE:FF",
                "route-to-nic-name": "Ethernet",
                "route-to-default-gateway": "192.168.1.1"
            }
        ]"#;
        let config = Config::from_json_str(json).unwrap();
        assert_eq!(config.rules[0].mac.as_deref(), Some("AA:BB:CC:DD:EE:FF"));
        assert_eq!(config.rules[0].nic.as_deref(), Some("Ethernet"));
        assert_eq!(config.rules[0].gateway.as_deref(), Some("192.168.1.1"));
    }

    fn make_entry(if_index: u32, gw: Ipv4Addr) -> MacEntry {
        MacEntry { if_index, gateway: gw, egress_ipv4: None }
    }

    #[test]
    fn test_compile_mac_wins_over_nic_and_gateway() {
        let mut config = Config::new();
        config
            .add_rule(
                "10.0.0.0/8".to_string(),
                Some("AA:BB:CC:DD:EE:FF".to_string()),
                Some("Wi-Fi".to_string()),
                Some("192.168.1.1".to_string()),
                None,
            )
            .unwrap();
        let mut mac_map = HashMap::new();
        mac_map.insert("AA:BB:CC:DD:EE:FF".to_string(), make_entry(42, Ipv4Addr::new(10, 0, 0, 1)));
        let mut nic_map = HashMap::new();
        nic_map.insert("wi-fi".to_string(), make_entry(3, Ipv4Addr::new(10, 0, 0, 1)));
        let mut gw_map = HashMap::new();
        gw_map.insert(Ipv4Addr::new(192, 168, 1, 1), make_entry(7, Ipv4Addr::new(192, 168, 1, 1)));
        let compiled = config.compile_rules(&mac_map, &nic_map, &gw_map).unwrap();
        assert_eq!(compiled[0].if_index, 42);
    }

    #[test]
    fn test_compile_nic_wins_over_gateway() {
        let mut config = Config::new();
        config
            .add_rule(
                "10.0.0.0/8".to_string(),
                None,
                Some("Ethernet".to_string()),
                Some("192.168.1.1".to_string()),
                None,
            )
            .unwrap();
        let mac_map = HashMap::new();
        let mut nic_map = HashMap::new();
        nic_map.insert("ethernet".to_string(), make_entry(9, Ipv4Addr::new(10, 0, 0, 1)));
        let mut gw_map = HashMap::new();
        gw_map.insert(Ipv4Addr::new(192, 168, 1, 1), make_entry(7, Ipv4Addr::new(192, 168, 1, 1)));
        let compiled = config.compile_rules(&mac_map, &nic_map, &gw_map).unwrap();
        assert_eq!(compiled[0].if_index, 9);
    }

    #[test]
    fn test_compile_gateway_fallback() {
        let mut config = Config::new();
        config
            .add_rule("10.0.0.0/8".to_string(), None, None, Some("192.168.1.1".to_string()), None)
            .unwrap();
        let mut gw_map = HashMap::new();
        gw_map.insert(Ipv4Addr::new(192, 168, 1, 1), make_entry(5, Ipv4Addr::new(192, 168, 1, 1)));
        let compiled = config.compile_rules(&HashMap::new(), &HashMap::new(), &gw_map).unwrap();
        assert_eq!(compiled[0].if_index, 5);
    }

    #[test]
    fn test_find_compiled() {
        let mut config = Config::new();
        config
            .add_rule("192.168.1.0/24".to_string(), Some("AA:BB:CC:DD:EE:01".to_string()), None, None, None)
            .unwrap();
        let mut mac_map = HashMap::new();
        mac_map.insert("AA:BB:CC:DD:EE:01".to_string(), make_entry(1, Ipv4Addr::new(10, 0, 0, 1)));
        let compiled = config.compile_rules(&mac_map, &HashMap::new(), &HashMap::new()).unwrap();
        let hit = Config::find_compiled(&compiled, Ipv4Addr::new(192, 168, 1, 50));
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().if_index, 1);
    }
}
