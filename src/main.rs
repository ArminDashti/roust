mod cli;
mod config;
mod network;
mod core;

use anyhow::{anyhow, Result};
use cli::{parse_cli, Commands, NicCommands, RuleAction};
use config::{Config};
use network::enumerate_interfaces;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_default_env()
        .format_timestamp_secs()
        .init();

    let cli = parse_cli();

    // Determine config path
    let config_path = cli.config.unwrap_or_else(Config::default_config_path);

    match cli.command {
        Commands::Nics { action } => handle_nics_command(action)?,
        Commands::Add { action } => handle_add_rule(action, &config_path)?,
        Commands::Delete { action } => handle_delete_rule(action, &config_path)?,
        Commands::Edit { action } => handle_edit_rule(action, &config_path)?,
        Commands::Start => handle_start_command(&config_path)?,
        Commands::Stop => handle_stop_command()?,
        Commands::Restart => handle_restart_command(&config_path)?,
        Commands::Status => handle_status_command()?,
    }

    Ok(())
}

fn handle_nics_command(action: NicCommands) -> Result<()> {
    match action {
        NicCommands::List => {
            println!("\n{:<20} {:<40} {:<20} {:<15} {:<20}", "Name", "Description", "MAC Address", "Type", "IPv4 Address");
            println!("{}", "-".repeat(120));

            let interfaces = enumerate_interfaces()?;
            for nic in &interfaces {
                let ipv4 = nic.ipv4_address.as_deref().unwrap_or("N/A");
                println!(
                    "{:<20} {:<40} {:<20} {:<15} {:<20}",
                    nic.name, nic.display_name, nic.mac_address, nic.status, ipv4
                );
            }
            Ok(())
        }
    }
}

fn handle_add_rule(action: RuleAction, config_path: &PathBuf) -> Result<()> {
    if let RuleAction::Rule { ip, nic, file } = action {
        let mut config = Config::load(config_path).unwrap_or_else(|_| Config::new());

        if let Some(dest) = nic {
            let nics = enumerate_interfaces()?;
            if !nics.iter().any(|n| n.name.eq_ignore_ascii_case(&dest)) {
                return Err(anyhow!("Interface '{}' not found", dest));
            }

            if let Some(file_path) = file {
                let content = fs::read_to_string(&file_path)?;
                let ips: Vec<String> = if file_path.extension().map_or(false, |ext| ext == "json") {
                    serde_json::from_str(&content)?
                } else {
                    content
                        .lines()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                };

                for ip_str in ips {
                    config.add_rule(ip_str, dest.clone(), None)?;
                }
            } else if let Some(ip_addr) = ip {
                config.add_rule(ip_addr, dest, None)?;
            }
            config.save(config_path)?;
            println!("Rule(s) added successfully.");
        }
    }
    Ok(())
}

fn handle_delete_rule(action: RuleAction, config_path: &PathBuf) -> Result<()> {
    if let RuleAction::Rule { ip, .. } = action {
        if let Some(ip_addr) = ip {
            let mut config = Config::load(config_path)?;
            if config.remove_rule(&ip_addr) {
                config.save(config_path)?;
                println!("Rule deleted successfully.");
            } else {
                println!("Rule not found.");
            }
        }
    }
    Ok(())
}

fn handle_edit_rule(action: RuleAction, config_path: &PathBuf) -> Result<()> {
    if let RuleAction::Rule { ip, nic, .. } = action {
        if let (Some(ip_addr), Some(new_nic)) = (ip, nic) {
            let mut config = Config::load(config_path)?;
            config.remove_rule(&ip_addr);
            config.add_rule(ip_addr, new_nic, None)?;
            config.save(config_path)?;
            println!("Rule edited successfully.");
        }
    }
    Ok(())
}

fn handle_start_command(config_path: &PathBuf) -> Result<()> {
    let config = Config::load(config_path)?;
    println!("[INFO] Starting router with {} rules...", config.rules.len());
    // Implementation of router start...
    Ok(())
}

fn handle_stop_command() -> Result<()> {
    println!("[INFO] Router stopped.");
    Ok(())
}

fn handle_restart_command(config_path: &PathBuf) -> Result<()> {
    handle_stop_command()?;
    handle_start_command(config_path)?;
    Ok(())
}

fn handle_status_command() -> Result<()> {
    println!("[INFO] Router status: Not running (placeholder).");
    Ok(())
}
