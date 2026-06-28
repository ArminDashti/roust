use anyhow::{anyhow, Result};
use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};

/// Pre-parsed match pattern used on the packet hot path.
#[derive(Debug, Clone)]
pub enum MatchPattern {
    Network(IpNetwork),
    Ip(Ipv4Addr),
    Interface(u32),
}

/// One routing rule with gateway resolved to `if_index` at startup.
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub label: String,
    pub gateway: Ipv4Addr,
    pub match_pattern: MatchPattern,
    pub if_index: u32,
    pub egress_ipv4: Option<Ipv4Addr>,
}

/// Resolved interface info built at startup from live adapter enumeration.
#[derive(Debug, Clone)]
pub struct MacEntry {
    pub if_index: u32,
    pub gateway: Ipv4Addr,
    pub egress_ipv4: Option<Ipv4Addr>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Nic,
    Ip,
    Cidr,
    Mac,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DestinationKind {
    Nic,
    Ip,
    Mac,
}

/// A routing rule as stored in `routes.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingRule {
    pub target: TargetKind,
    #[serde(rename = "target-value")]
    pub target_value: String,
    pub destination: DestinationKind,
    #[serde(rename = "destination-value")]
    pub destination_value: String,
}

impl CompiledRule {
    /// Outbound packets match on destination IP; inbound packets match on source IP (remote peer).
    /// NIC/MAC targets match on the packet interface index from WinDivert.
    pub fn matches(&self, peer_ip: Ipv4Addr, if_idx: u32) -> bool {
        match &self.match_pattern {
            MatchPattern::Network(network) => network.contains(IpAddr::V4(peer_ip)),
            MatchPattern::Ip(ip) => *ip == peer_ip,
            MatchPattern::Interface(index) => *index == if_idx,
        }
    }
}

impl RoutingRule {
    pub fn label(&self) -> String {
        format!(
            "{}:{} → {}:{}",
            serde_json::to_value(self.target)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| "?".into()),
            self.target_value,
            serde_json::to_value(self.destination)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| "?".into()),
            self.destination_value
        )
    }

    pub fn validate(&self) -> Result<()> {
        validate_target(self.target, &self.target_value)?;
        validate_destination_value(self.destination, &self.destination_value)?;
        Ok(())
    }
}

fn ipv4_network(cidr: &str) -> Result<ipnetwork::Ipv4Network> {
    match validate_cidr(cidr)? {
        IpNetwork::V4(network) => Ok(network),
        _ => unreachable!("validate_cidr rejects non-IPv4 networks"),
    }
}

fn ipv4_ranges_overlap(a: &ipnetwork::Ipv4Network, b: &ipnetwork::Ipv4Network) -> bool {
    let a_start = u32::from(a.network());
    let a_end = u32::from(a.broadcast());
    let b_start = u32::from(b.network());
    let b_end = u32::from(b.broadcast());
    a_start <= b_end && b_start <= a_end
}

/// Reject duplicate or overlapping CIDR targets in a rule list.
pub fn validate_rules_cidr_targets(rules: &[RoutingRule]) -> Result<()> {
    let mut seen: Vec<(String, ipnetwork::Ipv4Network)> = Vec::new();

    for rule in rules {
        if rule.target != TargetKind::Cidr {
            continue;
        }
        let network = ipv4_network(&rule.target_value)?;
        let label = rule.label();

        for (existing_label, existing_network) in &seen {
            if network == *existing_network {
                return Err(anyhow!(
                    "duplicate CIDR target \"{}\" (same network as \"{existing_label}\")",
                    rule.target_value
                ));
            }
            if ipv4_ranges_overlap(&network, existing_network) {
                return Err(anyhow!(
                    "overlapping CIDR targets \"{}\" and \"{existing_label}\"",
                    rule.target_value,
                ));
            }
        }

        seen.push((label, network));
    }

    Ok(())
}

fn validate_new_cidr_target(cidr: &str, existing: &[RoutingRule]) -> Result<()> {
    let network = ipv4_network(cidr)?;

    for rule in existing {
        if rule.target != TargetKind::Cidr {
            continue;
        }
        let existing_network = ipv4_network(&rule.target_value)?;
        if network == existing_network {
            return Err(anyhow!(
                "duplicate CIDR target \"{cidr}\" (same network as \"{}\")",
                rule.target_value
            ));
        }
        if ipv4_ranges_overlap(&network, &existing_network) {
            return Err(anyhow!(
                "overlapping CIDR targets \"{cidr}\" and \"{}\"",
                rule.target_value,
            ));
        }
    }

    Ok(())
}

pub fn validate_cidr(cidr: &str) -> Result<IpNetwork> {
    let trimmed = cidr.trim();
    if trimmed == "*" {
        return Err(anyhow!(
            "CIDR target \"{trimmed}\" must be a CIDR block (e.g. 10.0.0.0/8); wildcards are not supported"
        ));
    }
    if !trimmed.contains('/') {
        return Err(anyhow!(
            "CIDR target \"{trimmed}\" must include a prefix (e.g. 10.0.0.0/8)"
        ));
    }
    let network = trimmed
        .parse::<IpNetwork>()
        .map_err(|e| anyhow!("invalid CIDR \"{trimmed}\": {e}"))?;
    match network {
        IpNetwork::V4(_) => Ok(network),
        _ => Err(anyhow!("CIDR target \"{trimmed}\" must be an IPv4 CIDR block")),
    }
}

pub fn validate_ipv4(ip: &str) -> Result<Ipv4Addr> {
    let trimmed = ip.trim();
    if trimmed.contains('/') {
        return Err(anyhow!(
            "IP target \"{trimmed}\" must be a plain IPv4 address, not a CIDR block"
        ));
    }
    trimmed
        .parse::<Ipv4Addr>()
        .map_err(|_| anyhow!("invalid IPv4 address \"{trimmed}\""))
}

fn is_mac_address(value: &str) -> bool {
    let normalized = value.replace('-', ":");
    let octets: Vec<&str> = normalized.split(':').collect();
    octets.len() == 6
        && octets
            .iter()
            .all(|octet| octet.len() == 2 && octet.chars().all(|ch| ch.is_ascii_hexdigit()))
}

fn normalize_mac(value: &str) -> String {
    value.replace('-', ":").to_ascii_uppercase()
}

pub fn validate_target(kind: TargetKind, value: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("target-value must not be empty"));
    }
    match kind {
        TargetKind::Cidr => {
            validate_cidr(trimmed)?;
        }
        TargetKind::Ip => {
            validate_ipv4(trimmed)?;
        }
        TargetKind::Mac => {
            if !is_mac_address(trimmed) {
                return Err(anyhow!(
                    "target-value \"{trimmed}\" is not a valid MAC address (expected AA:BB:CC:DD:EE:FF)"
                ));
            }
        }
        TargetKind::Nic => {}
    }
    Ok(())
}

pub fn validate_destination_value(kind: DestinationKind, value: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("destination-value must not be empty"));
    }
    match kind {
        DestinationKind::Ip => {
            validate_ipv4(trimmed)?;
        }
        DestinationKind::Mac => {
            if !is_mac_address(trimmed) {
                return Err(anyhow!(
                    "destination-value \"{trimmed}\" is not a valid MAC address (expected AA:BB:CC:DD:EE:FF)"
                ));
            }
        }
        DestinationKind::Nic => {}
    }
    Ok(())
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
            anyhow!(
                "invalid routes JSON (expected [{{\"target\":\"cidr\",\"target-value\":\"...\",\
                 \"destination\":\"ip\",\"destination-value\":\"...\"}}]): {e}"
            )
        })?;
        for rule in &rules {
            rule.validate()?;
        }
        validate_rules_cidr_targets(&rules)?;
        Ok(Config { rules })
    }

    pub fn parse_import_file(contents: &str, path: &Path) -> Result<Vec<RoutingRule>> {
        let is_json = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
        if !is_json {
            return Err(anyhow!(
                "import only supports JSON files with the new rule format \
                 (target, target-value, destination, destination-value)"
            ));
        }
        let rules: Vec<RoutingRule> = serde_json::from_str(contents).map_err(|e| {
            anyhow!(
                "JSON import must be an array of rule objects with target, target-value, \
                 destination, and destination-value: {e}"
            )
        })?;
        for rule in &rules {
            rule.validate()?;
        }
        validate_rules_cidr_targets(&rules)?;
        Ok(rules)
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

    pub fn add_rule(&mut self, rule: RoutingRule) -> Result<()> {
        rule.validate()?;
        if rule.target == TargetKind::Cidr {
            validate_new_cidr_target(&rule.target_value, &self.rules)?;
        }
        self.rules.push(rule);
        Ok(())
    }

    pub fn remove_rule_at(&mut self, index: usize) -> bool {
        if index >= self.rules.len() {
            return false;
        }
        self.rules.remove(index);
        true
    }

    pub fn replace_rule_at(&mut self, index: usize, rule: RoutingRule) -> Result<()> {
        if index >= self.rules.len() {
            return Err(anyhow!("rule index {index} not found"));
        }
        rule.validate()?;
        if rule.target == TargetKind::Cidr {
            let others: Vec<_> = self
                .rules
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, r)| r.clone())
                .collect();
            validate_new_cidr_target(&rule.target_value, &others)?;
        }
        self.rules[index] = rule;
        Ok(())
    }

    pub fn get_rules(&self) -> &[RoutingRule] {
        &self.rules
    }

    pub fn compile_rules(
        &self,
        mac_map: &HashMap<String, MacEntry>,
        nic_map: &HashMap<String, MacEntry>,
        gw_map: &HashMap<Ipv4Addr, MacEntry>,
    ) -> Result<Vec<CompiledRule>> {
        let mut compiled = Vec::with_capacity(self.rules.len());
        for rule in &self.rules {
            let match_pattern = Self::compile_target(rule.target, &rule.target_value, mac_map, nic_map)?;
            let entry = Self::resolve_destination(rule, mac_map, nic_map, gw_map)?;
            compiled.push(CompiledRule {
                label: rule.label(),
                gateway: entry.gateway,
                match_pattern,
                if_index: entry.if_index,
                egress_ipv4: entry.egress_ipv4,
            });
        }
        Ok(compiled)
    }

    fn resolve_destination<'a>(
        rule: &RoutingRule,
        mac_map: &'a HashMap<String, MacEntry>,
        nic_map: &'a HashMap<String, MacEntry>,
        gw_map: &'a HashMap<Ipv4Addr, MacEntry>,
    ) -> Result<&'a MacEntry> {
        let label = rule.label();
        match rule.destination {
            DestinationKind::Mac => {
                let mac = normalize_mac(&rule.destination_value);
                mac_map.get(&mac).ok_or_else(|| {
                    anyhow!(
                        "rule {label}: destination MAC {mac} not found on any local interface"
                    )
                })
            }
            DestinationKind::Nic => {
                let nic = rule.destination_value.trim();
                nic_map.get(&nic.to_ascii_lowercase()).ok_or_else(|| {
                    anyhow!(
                        "rule {label}: destination NIC \"{nic}\" not found on any local interface"
                    )
                })
            }
            DestinationKind::Ip => {
                let gw: Ipv4Addr = validate_ipv4(&rule.destination_value)?;
                gw_map.get(&gw).ok_or_else(|| {
                    anyhow!(
                        "rule {label}: destination IP {gw} is not a default gateway on any local interface"
                    )
                })
            }
        }
    }

    pub fn find_compiled<'a>(
        compiled: &'a [CompiledRule],
        peer_ip: Ipv4Addr,
        if_idx: u32,
    ) -> Option<&'a CompiledRule> {
        compiled.iter().find(|rule| rule.matches(peer_ip, if_idx))
    }

    fn compile_target(
        kind: TargetKind,
        value: &str,
        mac_map: &HashMap<String, MacEntry>,
        nic_map: &HashMap<String, MacEntry>,
    ) -> Result<MatchPattern> {
        let trimmed = value.trim();
        match kind {
            TargetKind::Cidr => Ok(MatchPattern::Network(validate_cidr(trimmed)?)),
            TargetKind::Ip => Ok(MatchPattern::Ip(validate_ipv4(trimmed)?)),
            TargetKind::Mac => {
                let mac = normalize_mac(trimmed);
                let entry = mac_map.get(&mac).ok_or_else(|| {
                    anyhow!(
                        "target MAC {mac} not found on any local interface"
                    )
                })?;
                Ok(MatchPattern::Interface(entry.if_index))
            }
            TargetKind::Nic => {
                let entry = nic_map.get(&trimmed.to_ascii_lowercase()).ok_or_else(|| {
                    anyhow!(
                        "target NIC \"{trimmed}\" not found on any local interface"
                    )
                })?;
                Ok(MatchPattern::Interface(entry.if_index))
            }
        }
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

    fn sample_rule() -> RoutingRule {
        RoutingRule {
            target: TargetKind::Cidr,
            target_value: "10.0.0.0/8".to_string(),
            destination: DestinationKind::Ip,
            destination_value: "192.168.1.1".to_string(),
        }
    }

    #[test]
    fn test_load_new_format() {
        let json = r#"[{
            "target": "cidr",
            "target-value": "10.0.0.0/8",
            "destination": "ip",
            "destination-value": "192.168.1.1"
        }]"#;
        let config = Config::from_json_str(json).unwrap();
        assert_eq!(config.rules[0].target, TargetKind::Cidr);
        assert_eq!(config.rules[0].destination, DestinationKind::Ip);
    }

    #[test]
    fn test_reject_missing_fields() {
        let json = r#"[{"target":"cidr","target-value":"10.0.0.0/8"}]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("invalid routes JSON"));
    }

    #[test]
    fn test_reject_plain_ip_as_cidr_target() {
        let json = r#"[{
            "target": "cidr",
            "target-value": "8.8.8.8",
            "destination": "ip",
            "destination-value": "192.168.1.1"
        }]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("prefix"));
    }

    #[test]
    fn test_reject_cidr_as_ip_target() {
        let json = r#"[{
            "target": "ip",
            "target-value": "8.8.8.8/32",
            "destination": "ip",
            "destination-value": "192.168.1.1"
        }]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("plain IPv4"));
    }

    #[test]
    fn test_reject_overlapping_cidr_targets() {
        let json = r#"[
            {
                "target": "cidr",
                "target-value": "10.0.0.0/8",
                "destination": "ip",
                "destination-value": "192.168.1.1"
            },
            {
                "target": "cidr",
                "target-value": "10.1.0.0/16",
                "destination": "ip",
                "destination-value": "192.168.1.2"
            }
        ]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("overlapping CIDR"));
    }

    fn make_entry(if_index: u32, gw: Ipv4Addr) -> MacEntry {
        MacEntry {
            if_index,
            gateway: gw,
            egress_ipv4: None,
        }
    }

    #[test]
    fn test_compile_mac_destination() {
        let mut config = Config::new();
        config.add_rule(sample_rule()).unwrap();
        let mut mac_map = HashMap::new();
        mac_map.insert(
            "AA:BB:CC:DD:EE:FF".to_string(),
            make_entry(42, Ipv4Addr::new(10, 0, 0, 1)),
        );
        let mut gw_map = HashMap::new();
        gw_map.insert(Ipv4Addr::new(192, 168, 1, 1), make_entry(42, Ipv4Addr::new(192, 168, 1, 1)));
        let compiled = config
            .compile_rules(&mac_map, &HashMap::new(), &gw_map)
            .unwrap();
        assert_eq!(compiled[0].if_index, 42);
    }

    #[test]
    fn test_compile_nic_destination() {
        let mut config = Config::new();
        config
            .add_rule(RoutingRule {
                target: TargetKind::Cidr,
                target_value: "10.0.0.0/8".to_string(),
                destination: DestinationKind::Nic,
                destination_value: "Ethernet".to_string(),
            })
            .unwrap();
        let mut nic_map = HashMap::new();
        nic_map.insert(
            "ethernet".to_string(),
            make_entry(9, Ipv4Addr::new(10, 0, 0, 1)),
        );
        let compiled = config
            .compile_rules(&HashMap::new(), &nic_map, &HashMap::new())
            .unwrap();
        assert_eq!(compiled[0].if_index, 9);
    }

    #[test]
    fn test_find_compiled_cidr() {
        let mut config = Config::new();
        config
            .add_rule(RoutingRule {
                target: TargetKind::Cidr,
                target_value: "192.168.1.0/24".to_string(),
                destination: DestinationKind::Mac,
                destination_value: "AA:BB:CC:DD:EE:01".to_string(),
            })
            .unwrap();
        let mut mac_map = HashMap::new();
        mac_map.insert(
            "AA:BB:CC:DD:EE:01".to_string(),
            make_entry(1, Ipv4Addr::new(10, 0, 0, 1)),
        );
        let compiled = config
            .compile_rules(&mac_map, &HashMap::new(), &HashMap::new())
            .unwrap();
        let hit = Config::find_compiled(&compiled, Ipv4Addr::new(192, 168, 1, 50), 0);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().if_index, 1);
    }

    #[test]
    fn test_find_compiled_interface_target() {
        let mut config = Config::new();
        config
            .add_rule(RoutingRule {
                target: TargetKind::Nic,
                target_value: "Wi-Fi".to_string(),
                destination: DestinationKind::Mac,
                destination_value: "AA:BB:CC:DD:EE:01".to_string(),
            })
            .unwrap();
        let mut nic_map = HashMap::new();
        nic_map.insert("wi-fi".to_string(), make_entry(7, Ipv4Addr::new(10, 0, 0, 1)));
        let mut mac_map = HashMap::new();
        mac_map.insert(
            "AA:BB:CC:DD:EE:01".to_string(),
            make_entry(1, Ipv4Addr::new(10, 0, 0, 1)),
        );
        let compiled = config
            .compile_rules(&mac_map, &nic_map, &HashMap::new())
            .unwrap();
        let hit = Config::find_compiled(&compiled, Ipv4Addr::new(1, 2, 3, 4), 7);
        assert!(hit.is_some());
        let miss = Config::find_compiled(&compiled, Ipv4Addr::new(1, 2, 3, 4), 3);
        assert!(miss.is_none());
    }
}
