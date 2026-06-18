//! Shared application logic for the GUI frontend.

use crate::config::{Config, DestinationKind, RoutingRule, TargetKind};
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

fn validate_rule_on_host(rule: &RoutingRule) -> Result<()> {
    rule.validate()?;
    let interfaces = enumerate_interfaces()?;
    let (mac_map, nic_map, gw_map) = build_adapter_maps(&interfaces);
    Config::compile_rules(
        &Config {
            rules: vec![rule.clone()],
        },
        &mac_map,
        &nic_map,
        &gw_map,
    )?;
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
    target: TargetKind,
    target_value: String,
    destination: DestinationKind,
    destination_value: String,
) -> Result<RuleMutationResult> {
    let rule = RoutingRule {
        target,
        target_value: target_value.trim().to_string(),
        destination,
        destination_value: destination_value.trim().to_string(),
    };
    validate_rule_on_host(&rule)?;
    let mut config = Config::load(config_path).unwrap_or_else(|_| Config::new());
    config.add_rule(rule)?;
    config.save(config_path)?;
    Ok(RuleMutationResult {
        message: "Rule added successfully.".into(),
        live_apply_hint: live_apply_hint(),
    })
}

pub fn import_rules_from_file(
    config_path: &Path,
    file_path: &Path,
) -> Result<RuleMutationResult> {
    let content = fs::read_to_string(file_path)?;
    let imported = Config::parse_import_file(&content, file_path)?;
    let mut config = Config::load(config_path).unwrap_or_else(|_| Config::new());

    for rule in imported {
        validate_rule_on_host(&rule).map_err(|e| {
            anyhow!("entry {} in {}: {e}", rule.label(), file_path.display())
        })?;
        config.add_rule(rule)?;
    }

    config.save(config_path)?;
    Ok(RuleMutationResult {
        message: "Rule(s) imported successfully.".into(),
        live_apply_hint: live_apply_hint(),
    })
}

pub fn delete_rule(config_path: &Path, index: usize) -> Result<RuleMutationResult> {
    let mut config = Config::load(config_path)?;
    if config.remove_rule_at(index) {
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
    index: usize,
    target: TargetKind,
    target_value: String,
    destination: DestinationKind,
    destination_value: String,
) -> Result<RuleMutationResult> {
    let rule = RoutingRule {
        target,
        target_value: target_value.trim().to_string(),
        destination,
        destination_value: destination_value.trim().to_string(),
    };
    validate_rule_on_host(&rule)?;
    let mut config = Config::load(config_path)?;
    config.replace_rule_at(index, rule)?;
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
