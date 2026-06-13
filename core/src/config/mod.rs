use anyhow::{anyhow, Result};
use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};

/// Pre-parsed CIDR match pattern used on the packet hot path (no JSON or CIDR parsing per packet).
#[derive(Debug, Clone)]
pub enum IpMatch {
    Network(IpNetwork),
}

/// One routing rule with gateway resolved to `if_index` at startup; kept entirely in memory while running.
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub cidr_label: String,
    pub gateway: Ipv4Addr,
    pub match_pattern: IpMatch,
    pub if_index: u32,
    /// Primary IPv4 on the target interface; used to rewrite outbound source when redirecting egress.
    pub egress_ipv4: Option<Ipv4Addr>,
}

/// Resolved interface info built at startup from live adapter enumeration.
#[derive(Debug, Clone)]
pub struct MacEntry {
    pub if_index: u32,
    pub gateway: Ipv4Addr,
    pub egress_ipv4: Option<Ipv4Addr>,
}

/// Classified routing target parsed from a single `rewrite-to` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteTargetKind {
    Mac(String),
    Nic(String),
    Gateway(String),
}

impl CompiledRule {
    /// Return true when this rule's pattern matches the given IPv4 address.
    /// Outbound packets match on destination; inbound packets match on source (remote peer).
    pub fn matches(&self, dest: Ipv4Addr) -> bool {
        match &self.match_pattern {
            IpMatch::Network(network) => network.contains(IpAddr::V4(dest)),
        }
    }
}

/// A routing rule as stored in `routes.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingRule {
    pub cidr: String,
    #[serde(rename = "rewrite-to")]
    pub rewrite_to: String,
}

/// Input struct for loading `routes.json`; accepts legacy rewrite-target field names only.
#[derive(Debug, Deserialize)]
struct RoutingRuleInput {
    cidr: String,
    #[serde(rename = "rewrite-to", alias = "rewrite_to", default)]
    rewrite_to: Option<String>,
    #[serde(rename = "route-to-mac-address", default)]
    mac: Option<String>,
    #[serde(rename = "route-to-nic-name", default)]
    nic: Option<String>,
    #[serde(rename = "route-to-default-gateway", alias = "gateway", default)]
    gateway: Option<String>,
    #[serde(rename = "nic", default)]
    legacy_nic: Option<String>,
}

impl RoutingRuleInput {
    fn into_rule(self) -> Result<RoutingRule> {
        if let Some(legacy_nic) = self.legacy_nic {
            return Err(anyhow!(
                "routing rule {} uses deprecated field \"nic\"; use \"rewrite-to\" with a MAC address, \
                 NIC name, or gateway IP instead (got \"{legacy_nic}\")",
                self.cidr
            ));
        }

        let rewrite_to = if let Some(value) = self
            .rewrite_to
            .filter(|value| !value.trim().is_empty())
        {
            value.trim().to_string()
        } else if let Some(mac) = self.mac.filter(|value| !value.is_empty()) {
            mac.trim().to_string()
        } else if let Some(nic) = self.nic.filter(|value| !value.is_empty()) {
            nic.trim().to_string()
        } else if let Some(gateway) = self.gateway.filter(|value| !value.is_empty()) {
            gateway.trim().to_string()
        } else {
            return Err(anyhow!(
                "routing rule {} must set \"rewrite-to\" to a MAC address, NIC name, or gateway IP \
                 (see Adapters in the Roust app).",
                self.cidr
            ));
        };

        validate_cidr(&self.cidr)?;
        Ok(RoutingRule {
            cidr: self.cidr,
            rewrite_to,
        })
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

/// Reject duplicate or overlapping CIDR blocks in a rule list (e.g. routes JSON).
pub fn validate_rules_cidrs(rules: &[RoutingRule]) -> Result<()> {
    let mut seen: Vec<(String, ipnetwork::Ipv4Network)> = Vec::with_capacity(rules.len());

    for rule in rules {
        let network = ipv4_network(&rule.cidr)?;

        for (existing_label, existing_network) in &seen {
            if network == *existing_network {
                return Err(anyhow!(
                    "duplicate CIDR \"{}\" (same network as \"{existing_label}\")",
                    rule.cidr
                ));
            }
            if ipv4_ranges_overlap(&network, existing_network) {
                return Err(anyhow!(
                    "overlapping CIDR blocks \"{}\" and \"{existing_label}\"",
                    rule.cidr,
                ));
            }
        }

        seen.push((rule.cidr.clone(), network));
    }

    Ok(())
}

/// Reject a new rule when its CIDR duplicates or overlaps an existing rule.
pub fn validate_new_rule_cidr(cidr: &str, existing: &[RoutingRule]) -> Result<()> {
    let network = ipv4_network(cidr)?;

    for rule in existing {
        let existing_network = ipv4_network(&rule.cidr)?;
        if network == existing_network {
            return Err(anyhow!(
                "duplicate CIDR \"{cidr}\" (same network as \"{}\")",
                rule.cidr
            ));
        }
        if ipv4_ranges_overlap(&network, &existing_network) {
            return Err(anyhow!(
                "overlapping CIDR blocks \"{cidr}\" and \"{}\"",
                rule.cidr,
            ));
        }
    }

    Ok(())
}

/// Validate that a rule match value is an IPv4 CIDR block (not a plain IP or wildcard).
pub fn validate_cidr(cidr: &str) -> Result<IpNetwork> {
    let trimmed = cidr.trim();
    if trimmed == "*" {
        return Err(anyhow!(
            "routing rule \"{trimmed}\" must be a CIDR block (e.g. 10.0.0.0/8); wildcards are not supported"
        ));
    }
    if !trimmed.contains('/') {
        return Err(anyhow!(
            "routing rule \"{trimmed}\" must be a CIDR block (e.g. 10.0.0.0/8), not a plain IP address"
        ));
    }
    let network = trimmed
        .parse::<IpNetwork>()
        .map_err(|e| anyhow!("invalid CIDR \"{trimmed}\": {e}"))?;
    match network {
        IpNetwork::V4(_) => Ok(network),
        _ => Err(anyhow!("routing rule \"{trimmed}\" must be an IPv4 CIDR block")),
    }
}

/// Detect whether `rewrite-to` is a MAC address, NIC name, or gateway IPv4.
///
/// Priority: IPv4 gateway > MAC pattern > NIC name.
pub fn classify_rewrite_target(value: &str) -> Result<RewriteTargetKind> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("rewrite-to must not be empty"));
    }

    if trimmed.parse::<Ipv4Addr>().is_ok() {
        return Ok(RewriteTargetKind::Gateway(trimmed.to_string()));
    }

    if is_mac_address(trimmed) {
        return Ok(RewriteTargetKind::Mac(normalize_mac(trimmed)));
    }

    Ok(RewriteTargetKind::Nic(trimmed.to_string()))
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
                 [{{\"cidr\":\"...\",\"rewrite-to\":\"...\"}}]): {e}"
            )
        })?;
        let rules = inputs
            .into_iter()
            .map(RoutingRuleInput::into_rule)
            .collect::<Result<Vec<_>>>()?;
        validate_rules_cidrs(&rules)?;
        Ok(Config { rules })
    }

    pub fn parse_import_file(contents: &str, path: &Path) -> Result<Vec<RoutingRule>> {
        let is_json = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
        if is_json {
            if let Ok(rules) = serde_json::from_str::<Vec<RoutingRule>>(contents) {
                for rule in &rules {
                    validate_cidr(&rule.cidr)?;
                }
                validate_rules_cidrs(&rules)?;
                return Ok(rules);
            }
            let cidrs: Vec<String> = serde_json::from_str(contents).map_err(|e| {
                anyhow!(
                    "JSON import must be [\"cidr\", ...] or \
                     [{{\"cidr\":\"...\",\"rewrite-to\":\"...\"}}]: {e}"
                )
            })?;
            let rules: Vec<RoutingRule> = cidrs
                .into_iter()
                .map(|cidr| {
                    validate_cidr(&cidr)?;
                    Ok(RoutingRule {
                        cidr,
                        rewrite_to: String::new(),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            validate_rules_cidrs(&rules)?;
            return Ok(rules);
        }
        let rules: Vec<RoutingRule> = contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| {
                validate_cidr(line)?;
                Ok(RoutingRule {
                    cidr: line.to_string(),
                    rewrite_to: String::new(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        validate_rules_cidrs(&rules)?;
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

    pub fn add_rule(&mut self, cidr: String, rewrite_to: String) -> Result<()> {
        validate_cidr(&cidr)?;
        let rewrite_to = rewrite_to.trim().to_string();
        if rewrite_to.is_empty() {
            return Err(anyhow!(
                "rewrite-to must be a MAC address, NIC name, or gateway IP"
            ));
        }
        classify_rewrite_target(&rewrite_to)?;
        validate_new_rule_cidr(&cidr, &self.rules)?;
        self.rules.push(RoutingRule { cidr, rewrite_to });
        Ok(())
    }

    pub fn remove_rule(&mut self, cidr: &str) -> bool {
        let initial_len = self.rules.len();
        self.rules.retain(|rule| rule.cidr != cidr);
        self.rules.len() < initial_len
    }

    pub fn get_rules(&self) -> &[RoutingRule] {
        &self.rules
    }

    /// Build an in-memory routing table from `rewrite-to` targets.
    pub fn compile_rules(
        &self,
        mac_map: &HashMap<String, MacEntry>,
        nic_map: &HashMap<String, MacEntry>,
        gw_map: &HashMap<Ipv4Addr, MacEntry>,
    ) -> Result<Vec<CompiledRule>> {
        let mut compiled = Vec::with_capacity(self.rules.len());
        for rule in &self.rules {
            let match_pattern = Self::compile_match_pattern(&rule.cidr)?;
            let entry = Self::resolve_entry(rule, mac_map, nic_map, gw_map)?;
            compiled.push(CompiledRule {
                cidr_label: rule.cidr.clone(),
                gateway: entry.gateway,
                match_pattern,
                if_index: entry.if_index,
                egress_ipv4: entry.egress_ipv4,
            });
        }
        Ok(compiled)
    }

    fn resolve_entry<'a>(
        rule: &RoutingRule,
        mac_map: &'a HashMap<String, MacEntry>,
        nic_map: &'a HashMap<String, MacEntry>,
        gw_map: &'a HashMap<Ipv4Addr, MacEntry>,
    ) -> Result<&'a MacEntry> {
        match classify_rewrite_target(&rule.rewrite_to)? {
            RewriteTargetKind::Mac(mac) => mac_map.get(&mac).ok_or_else(|| {
                anyhow!(
                    "routing rule {} → MAC {} not found on any local interface",
                    rule.cidr, mac
                )
            }),
            RewriteTargetKind::Nic(nic) => nic_map.get(&nic.to_ascii_lowercase()).ok_or_else(|| {
                anyhow!(
                    "routing rule {} → NIC name \"{}\" not found on any local interface",
                    rule.cidr, nic
                )
            }),
            RewriteTargetKind::Gateway(gateway) => {
                let gw: Ipv4Addr = gateway.parse().map_err(|e| {
                    anyhow!(
                        "routing rule {} has invalid gateway \"{gateway}\": {e}",
                        rule.cidr
                    )
                })?;
                gw_map.get(&gw).ok_or_else(|| {
                    anyhow!(
                        "routing rule {} → gateway {} is not a default gateway on any local interface",
                        rule.cidr, gateway
                    )
                })
            }
        }
    }

    /// Look up the first matching compiled rule for a destination IPv4 (in-memory only).
    pub fn find_compiled<'a>(
        compiled: &'a [CompiledRule],
        dest: Ipv4Addr,
    ) -> Option<&'a CompiledRule> {
        compiled.iter().find(|rule| rule.matches(dest))
    }

    fn compile_match_pattern(cidr: &str) -> Result<IpMatch> {
        Ok(IpMatch::Network(validate_cidr(cidr)?))
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
        let json = r#"[{"cidr":"10.0.0.0/8","nic":"Ethernet"}]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("deprecated"));
    }

    #[test]
    fn test_reject_rule_without_rewrite_to() {
        let json = r#"[{"cidr":"10.0.0.0/8"}]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("rewrite-to"));
    }

    #[test]
    fn test_reject_plain_ip() {
        let json = r#"[{"cidr":"8.8.8.8","rewrite-to":"192.168.1.1"}]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("CIDR"));
    }

    #[test]
    fn test_reject_wildcard() {
        let json = r#"[{"cidr":"*","rewrite-to":"192.168.1.1"}]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("CIDR"));
    }

    #[test]
    fn test_load_rewrite_to_mac() {
        let json = r#"[{"cidr": "10.0.0.0/8", "rewrite-to": "AA:BB:CC:DD:EE:FF"}]"#;
        let config = Config::from_json_str(json).unwrap();
        assert_eq!(config.rules[0].rewrite_to, "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn test_load_rewrite_to_nic() {
        let json = r#"[{"cidr": "10.0.0.0/8", "rewrite-to": "Wi-Fi"}]"#;
        let config = Config::from_json_str(json).unwrap();
        assert_eq!(config.rules[0].rewrite_to, "Wi-Fi");
    }

    #[test]
    fn test_load_rewrite_to_gateway() {
        let json = r#"[{"cidr": "10.0.0.0/8", "rewrite-to": "192.168.1.1"}]"#;
        let config = Config::from_json_str(json).unwrap();
        assert_eq!(config.rules[0].rewrite_to, "192.168.1.1");
    }

    #[test]
    fn test_reject_legacy_ip_or_cidr_field() {
        let json = r#"[{"ip-or-cidr": "10.0.0.0/8", "rewrite-to": "192.168.1.1"}]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("invalid routes JSON"));
    }

    #[test]
    fn test_reject_legacy_ip_field() {
        let json = r#"[{"ip": "10.0.0.0/8", "rewrite-to": "192.168.1.1"}]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("invalid routes JSON"));
    }

    #[test]
    fn test_load_legacy_route_to_fields() {
        let json = r#"[
            {"cidr": "10.0.0.0/8", "route-to-mac-address": "AA:BB:CC:DD:EE:FF"},
            {"cidr": "172.16.0.0/12", "route-to-nic-name": "Ethernet"},
            {"cidr": "192.168.0.0/16", "route-to-default-gateway": "192.168.1.1"}
        ]"#;
        let config = Config::from_json_str(json).unwrap();
        assert_eq!(config.rules[0].rewrite_to, "AA:BB:CC:DD:EE:FF");
        assert_eq!(config.rules[1].rewrite_to, "Ethernet");
        assert_eq!(config.rules[2].rewrite_to, "192.168.1.1");
    }

    #[test]
    fn test_reject_duplicate_cidr_in_json() {
        let json = r#"[
            {"cidr": "10.0.0.0/8", "rewrite-to": "192.168.1.1"},
            {"cidr": "10.0.0.0/8", "rewrite-to": "192.168.1.2"}
        ]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("duplicate CIDR"));
    }

    #[test]
    fn test_reject_equivalent_cidr_in_json() {
        let json = r#"[
            {"cidr": "10.0.0.0/8", "rewrite-to": "192.168.1.1"},
            {"cidr": "10.0.0.1/8", "rewrite-to": "192.168.1.2"}
        ]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("duplicate CIDR"));
    }

    #[test]
    fn test_reject_overlapping_cidr_in_json() {
        let json = r#"[
            {"cidr": "10.0.0.0/8", "rewrite-to": "192.168.1.1"},
            {"cidr": "10.1.0.0/16", "rewrite-to": "192.168.1.2"}
        ]"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("overlapping CIDR"));
    }

    #[test]
    fn test_reject_overlapping_cidr_on_add_rule() {
        let mut config = Config::new();
        config
            .add_rule("10.0.0.0/8".to_string(), "192.168.1.1".to_string())
            .unwrap();
        let err = config
            .add_rule("10.1.0.0/16".to_string(), "192.168.1.2".to_string())
            .unwrap_err();
        assert!(err.to_string().contains("overlapping CIDR"));
    }

    #[test]
    fn test_classify_rewrite_target() {
        assert_eq!(
            classify_rewrite_target("192.168.1.1").unwrap(),
            RewriteTargetKind::Gateway("192.168.1.1".into())
        );
        assert_eq!(
            classify_rewrite_target("AA-BB-CC-DD-EE-FF").unwrap(),
            RewriteTargetKind::Mac("AA:BB:CC:DD:EE:FF".into())
        );
        assert_eq!(
            classify_rewrite_target("Ethernet").unwrap(),
            RewriteTargetKind::Nic("Ethernet".into())
        );
    }

    fn make_entry(if_index: u32, gw: Ipv4Addr) -> MacEntry {
        MacEntry { if_index, gateway: gw, egress_ipv4: None }
    }

    #[test]
    fn test_compile_mac_target() {
        let mut config = Config::new();
        config
            .add_rule("10.0.0.0/8".to_string(), "AA:BB:CC:DD:EE:FF".to_string())
            .unwrap();
        let mut mac_map = HashMap::new();
        mac_map.insert("AA:BB:CC:DD:EE:FF".to_string(), make_entry(42, Ipv4Addr::new(10, 0, 0, 1)));
        let compiled = config.compile_rules(&mac_map, &HashMap::new(), &HashMap::new()).unwrap();
        assert_eq!(compiled[0].if_index, 42);
    }

    #[test]
    fn test_compile_nic_target() {
        let mut config = Config::new();
        config
            .add_rule("10.0.0.0/8".to_string(), "Ethernet".to_string())
            .unwrap();
        let mut nic_map = HashMap::new();
        nic_map.insert("ethernet".to_string(), make_entry(9, Ipv4Addr::new(10, 0, 0, 1)));
        let compiled = config.compile_rules(&HashMap::new(), &nic_map, &HashMap::new()).unwrap();
        assert_eq!(compiled[0].if_index, 9);
    }

    #[test]
    fn test_compile_gateway_target() {
        let mut config = Config::new();
        config
            .add_rule("10.0.0.0/8".to_string(), "192.168.1.1".to_string())
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
            .add_rule("192.168.1.0/24".to_string(), "AA:BB:CC:DD:EE:01".to_string())
            .unwrap();
        let mut mac_map = HashMap::new();
        mac_map.insert("AA:BB:CC:DD:EE:01".to_string(), make_entry(1, Ipv4Addr::new(10, 0, 0, 1)));
        let compiled = config.compile_rules(&mac_map, &HashMap::new(), &HashMap::new()).unwrap();
        let hit = Config::find_compiled(&compiled, Ipv4Addr::new(192, 168, 1, 50));
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().if_index, 1);
    }
}
