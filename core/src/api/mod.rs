//! Shared application logic for CLI and GUI frontends.

use crate::config::{Config, RoutingRule};
use crate::network::{build_adapter_maps, enumerate_interfaces, predict_ipv4_egress, EgressPrediction};
use std::net::Ipv4Addr;
use crate::service;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ServiceStatus {
    pub state: String,
    pub installed: bool,
    pub directory: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GatewayRow {
    pub nic_name: String,
    pub mac: String,
    pub gateway_ip: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PredictResult {
    pub destination: String,
    pub if_index: u32,
    pub next_hop: String,
    pub nic_name: Option<String>,
    pub nic_display: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleMutationResult {
    pub message: String,
    pub live_apply_hint: Option<String>,
}

pub fn resolve_config_path(config: Option<PathBuf>) -> PathBuf {
    config.unwrap_or_else(Config::default_config_path)
}

pub fn list_rules(config_path: &Path) -> Result<Vec<RoutingRule>> {
    if !config_path.exists() {
        return Ok(vec![]);
    }
    Ok(Config::load(config_path)?.get_rules().to_vec())
}

pub fn list_gateways() -> Result<Vec<GatewayRow>> {
    let interfaces = enumerate_interfaces()?;
    let mut rows = Vec::new();
    for nic in &interfaces {
        let gateway = nic
            .default_gateway
            .or_else(|| crate::network::gateway_from_forward_table(nic.if_index).ok());
        let Some(gateway) = gateway else {
            continue;
        };
        let nic_name = nic
            .friendly_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .or_else(|| {
                if nic.display_name.is_empty() {
                    None
                } else {
                    Some(nic.display_name.as_str())
                }
            })
            .unwrap_or(&nic.name);
        rows.push(GatewayRow {
            nic_name: nic_name.to_string(),
            mac: nic.mac_address.to_ascii_uppercase(),
            gateway_ip: gateway.to_string(),
        });
    }
    Ok(rows)
}

pub fn predict_route(ip: &str) -> Result<PredictResult> {
    let dest: Ipv4Addr = ip
        .parse()
        .map_err(|_| anyhow!("IP must be a valid IPv4 address (e.g. 8.8.8.8)"))?;
    let p = predict_ipv4_egress(dest)?;
    Ok(predict_result_from_egress(&p))
}

fn predict_result_from_egress(p: &EgressPrediction) -> PredictResult {
    PredictResult {
        destination: p.dest.to_string(),
        if_index: p.if_index,
        next_hop: p.next_hop.to_string(),
        nic_name: p.nic_name.clone().or_else(|| p.nic_friendly.clone()),
        nic_display: p.nic_display.clone(),
    }
}

/// Validate that at least one target field is provided and each value exists on a local adapter.
fn validate_targets(mac: Option<&str>, nic: Option<&str>, gateway: Option<&str>) -> Result<()> {
    if mac.is_none() && nic.is_none() && gateway.is_none() {
        return Err(anyhow!(
            "provide at least one of --mac, --nic, or --gateway \
             (run `roust gateway list` to see adapters)"
        ));
    }
    let interfaces = enumerate_interfaces()?;
    let (mac_map, nic_map, gw_map) = build_adapter_maps(&interfaces);
    if let Some(m) = mac {
        if !mac_map.contains_key(&m.to_ascii_uppercase()) {
            return Err(anyhow!(
                "MAC '{m}' not found on any local interface \
                 (run `roust gateway list` to see MACs)"
            ));
        }
    }
    if let Some(n) = nic {
        if !nic_map.contains_key(&n.to_ascii_lowercase()) {
            return Err(anyhow!(
                "NIC name '{n}' not found on any local interface \
                 (run `roust gateway list` to see names)"
            ));
        }
    }
    if let Some(g) = gateway {
        let gw: Ipv4Addr = g.parse().map_err(|_| anyhow!("invalid gateway IP '{g}'"))?;
        if !gw_map.contains_key(&gw) {
            return Err(anyhow!(
                "gateway '{g}' is not a default gateway on any local interface \
                 (run `roust gateway list` to see gateways)"
            ));
        }
    }
    Ok(())
}

fn live_apply_hint() -> Option<String> {
    match service::is_active() {
        Ok(true) => Some("Changes apply automatically to the running service.".into()),
        Ok(false) => Some("Start the service to apply rules.".into()),
        Err(_) => None,
    }
}

pub fn add_rule(
    config_path: &Path,
    ip: String,
    mac: Option<String>,
    nic: Option<String>,
    gateway: Option<String>,
    rewrite_to: Option<String>,
) -> Result<RuleMutationResult> {
    validate_targets(mac.as_deref(), nic.as_deref(), gateway.as_deref())?;
    let mut config = Config::load(config_path).unwrap_or_else(|_| Config::new());
    config.add_rule(ip, mac, nic, gateway, rewrite_to)?;
    config.save(config_path)?;
    Ok(RuleMutationResult {
        message: "Rule added successfully.".into(),
        live_apply_hint: live_apply_hint(),
    })
}

pub fn import_rules_from_file(
    config_path: &Path,
    file_path: &Path,
    default_mac: Option<String>,
    default_nic: Option<String>,
    default_gateway: Option<String>,
    rewrite_to: Option<String>,
) -> Result<RuleMutationResult> {
    let content = fs::read_to_string(file_path)?;
    let imported = Config::parse_import_file(&content, file_path)?;
    let mut config = Config::load(config_path).unwrap_or_else(|_| Config::new());

    for rule in imported {
        let mac = rule.mac.or_else(|| default_mac.clone());
        let nic = rule.nic.or_else(|| default_nic.clone());
        let gateway = rule.gateway.or_else(|| default_gateway.clone());
        validate_targets(mac.as_deref(), nic.as_deref(), gateway.as_deref()).map_err(|e| {
            anyhow!("entry {} in {}: {e}", rule.ip, file_path.display())
        })?;
        let rule_rewrite = rule.rewrite_to.or_else(|| rewrite_to.clone());
        config.add_rule(rule.ip, mac, nic, gateway, rule_rewrite)?;
    }

    config.save(config_path)?;
    Ok(RuleMutationResult {
        message: "Rule(s) imported successfully.".into(),
        live_apply_hint: live_apply_hint(),
    })
}

pub fn delete_rule(config_path: &Path, ip: &str) -> Result<RuleMutationResult> {
    let mut config = Config::load(config_path)?;
    if config.remove_rule(ip) {
        config.save(config_path)?;
        Ok(RuleMutationResult {
            message: "Rule deleted successfully.".into(),
            live_apply_hint: live_apply_hint(),
        })
    } else {
        Ok(RuleMutationResult {
            message: "Rule not found.".into(),
            live_apply_hint: None,
        })
    }
}

pub fn edit_rule(
    config_path: &Path,
    ip: String,
    mac: Option<String>,
    nic: Option<String>,
    gateway: Option<String>,
    rewrite_to: Option<String>,
) -> Result<RuleMutationResult> {
    validate_targets(mac.as_deref(), nic.as_deref(), gateway.as_deref())?;
    let mut config = Config::load(config_path)?;
    config.remove_rule(&ip);
    config.add_rule(ip, mac, nic, gateway, rewrite_to)?;
    config.save(config_path)?;
    Ok(RuleMutationResult {
        message: "Rule updated successfully.".into(),
        live_apply_hint: live_apply_hint(),
    })
}

pub fn service_status() -> Result<ServiceStatus> {
    let directory = service::exe_install_dir()?.display().to_string();
    let installed = service::is_installed().unwrap_or(false);
    let state = if installed {
        if service::is_active().unwrap_or(false) {
            "started"
        } else {
            "stopped"
        }
    } else {
        "not_installed"
    };
    Ok(ServiceStatus {
        state: state.into(),
        installed,
        directory,
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

pub fn start_service() -> Result<String> {
    if !service::is_installed()? {
        return Err(anyhow!(
            "The packet router runs as a Windows service. Install it first (elevated PowerShell)."
        ));
    }
    service::start()?;
    Ok("Service started.".into())
}

pub fn stop_service() -> Result<String> {
    service::stop()?;
    Ok("Service stopped.".into())
}

pub fn restart_service() -> Result<String> {
    service::restart()?;
    Ok("Service restarted.".into())
}
