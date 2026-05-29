mod cli;
mod config;
mod core;
mod network;
mod service;

use anyhow::{anyhow, Context, Result};
use cli::{parse_cli, Commands, RuleAction, RuleCommands};
use config::Config;
use network::{
    build_routing_gateway_index_map, enumerate_interfaces, gateway_exists_on_host,
};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::Ipv4Addr;
use std::path::PathBuf;

fn main() -> Result<()> {
    if service::invoked_as_service() {
        return service::run_dispatcher();
    }

    env_logger::Builder::from_default_env()
        .format_timestamp_secs()
        .init();
    bootstrap_runtime_files().context("prepare settings runtime file")?;
    let cli = parse_cli();

    if cli.install_service {
        return service::install(false);
    }
    if cli.uninstall_service {
        return service::uninstall();
    }

    let config_path = cli.config.unwrap_or_else(Config::default_config_path);
    let command = cli
        .command
        .ok_or_else(|| anyhow!("missing subcommand (run roust --help)"))?;

    match command {
        Commands::Predict { ip } => handle_predict_command(&ip)?,
        Commands::Rule { action } => handle_rule_command(action, &config_path)?,
        Commands::Add { action } => handle_add_rule(action, &config_path)?,
        Commands::Delete { action } => handle_delete_rule(action, &config_path)?,
        Commands::Edit { action } => handle_edit_rule(action, &config_path)?,
        Commands::Start => handle_start_command()?,
        Commands::Stop => handle_stop_command()?,
        Commands::Restart => handle_restart_command()?,
        Commands::Status => handle_status_command()?,
    }
    Ok(())
}

fn bootstrap_runtime_files() -> Result<()> {
    let cwd = env::current_dir().context("resolve current directory for runtime bootstrap")?;
    let settings_path = cwd.join("settings.json");
    if !settings_path.exists() {
        let mut settings = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&settings_path)
            .with_context(|| format!("create {}", settings_path.display()))?;
        settings
            .write_all(b"{}\n")
            .with_context(|| format!("initialize {}", settings_path.display()))?;
    }
    Ok(())
}

fn handle_predict_command(ip: &str) -> Result<()> {
    let dest: Ipv4Addr = ip
        .parse()
        .map_err(|_| anyhow!("--ip must be a valid IPv4 address (e.g. 8.8.8.8)"))?;
    let p = network::predict_ipv4_egress(dest)?;
    println!("destination:  {}", p.dest);
    println!("if_index:     {}", p.if_index);
    println!("next_hop:     {}", p.next_hop);
    match (&p.nic_name, &p.nic_display) {
        (Some(name), Some(disp)) => {
            println!("nic (name):   {}", name);
            println!("nic (desc):   {}", disp);
        }
        (Some(name), None) => println!("nic (name):   {}", name),
        _ => println!(
            "nic:          (no adapter matched if_index {}; check GetAdaptersInfo vs route table)",
            p.if_index
        ),
    }
    Ok(())
}

fn handle_rule_command(action: RuleCommands, config_path: &PathBuf) -> Result<()> {
    match action {
        RuleCommands::List => {
            if !config_path.exists() {
                println!("No routing rules ({} not found).", config_path.display());
                return Ok(());
            }
            let config = Config::load(config_path)?;
            let rules = config.get_rules();
            if rules.is_empty() {
                println!("No routing rules in {}.", config_path.display());
                return Ok(());
            }
            println!("Rules from {} ({}):", config_path.display(), rules.len());
            println!(
                "\n{:<24} {:<18} {:<18}",
                "IP / CIDR", "Gateway", "rewrite_to"
            );
            println!("{}", "-".repeat(64));
            for rule in rules {
                let rewrite = rule.rewrite_to.as_deref().unwrap_or("-");
                println!(
                    "{:<24} {:<18} {:<18}",
                    rule.ip, rule.gateway, rewrite
                );
            }
        }
    }
    Ok(())
}

fn notify_live_apply() {
    match service::is_active() {
        Ok(true) => println!("Changes apply automatically to the running service."),
        Ok(false) => println!("Start the service with `roust start` to apply rules."),
        Err(_) => {}
    }
}

fn handle_add_rule(action: RuleAction, config_path: &PathBuf) -> Result<()> {
    let RuleAction::Rule {
        ip,
        gateway,
        file,
        rewrite_to,
    } = action;
    let mut config = Config::load(config_path).unwrap_or_else(|_| Config::new());
    let interfaces = enumerate_interfaces()?;
    let gateway_index_map = build_routing_gateway_index_map(&interfaces)?;
    let validate_gateway = |gateway_str: &str| -> Result<()> {
        let gw: Ipv4Addr = gateway_str
            .parse()
            .map_err(|_| anyhow!("Invalid gateway '{}'", gateway_str))?;
        if gateway_exists_on_host(gw, &gateway_index_map) {
            Ok(())
        } else {
            Err(anyhow!(
                "Gateway '{}' is not a default gateway on this machine (check ipconfig or route print)",
                gateway_str
            ))
        }
    };

    if let Some(file_path) = file {
        let content = fs::read_to_string(&file_path)?;
        let imported = Config::parse_import_file(&content, &file_path)?;
        for rule in imported {
            let dest = if rule.gateway.is_empty() {
                gateway.as_ref()
                    .ok_or_else(|| {
                        anyhow!(
                            "Each entry in {} needs a \"gateway\", or pass --gateway for IP-only lists",
                            file_path.display()
                        )
                    })?
                    .clone()
            } else {
                rule.gateway
            };
            validate_gateway(&dest)?;
            let rule_rewrite = rule.rewrite_to.or_else(|| rewrite_to.clone());
            config.add_rule(rule.ip, dest, rule_rewrite)?;
        }
    } else if let (Some(ip_addr), Some(dest)) = (ip, gateway) {
        validate_gateway(&dest)?;
        config.add_rule(ip_addr, dest, rewrite_to)?;
    } else {
        return Err(anyhow!(
            "Provide --ip and --gateway, or --file (e.g. routes.json with ip and gateway per entry)"
        ));
    }
    config.save(config_path)?;
    println!("Rule(s) added successfully.");
    notify_live_apply();
    Ok(())
}

fn handle_delete_rule(action: RuleAction, config_path: &PathBuf) -> Result<()> {
    let RuleAction::Rule { ip, .. } = action;
    if let Some(ip_addr) = ip {
        let mut config = Config::load(config_path)?;
        if config.remove_rule(&ip_addr) {
            config.save(config_path)?;
            println!("Rule deleted successfully.");
            notify_live_apply();
        } else {
            println!("Rule not found.");
        }
    }
    Ok(())
}

fn handle_edit_rule(action: RuleAction, config_path: &PathBuf) -> Result<()> {
    let RuleAction::Rule {
        ip,
        gateway,
        rewrite_to,
        ..
    } = action;
    if let (Some(ip_addr), Some(new_gateway)) = (ip, gateway) {
        let interfaces = enumerate_interfaces()?;
        let gateway_index_map = build_routing_gateway_index_map(&interfaces)?;
        let gw: Ipv4Addr = new_gateway
            .parse()
            .map_err(|_| anyhow!("Invalid gateway '{}'", new_gateway))?;
        if !gateway_exists_on_host(gw, &gateway_index_map) {
            return Err(anyhow!(
                "Gateway '{}' is not a default gateway on this machine (check ipconfig or route print)",
                new_gateway
            ));
        }
        let mut config = Config::load(config_path)?;
        config.remove_rule(&ip_addr);
        config.add_rule(ip_addr, new_gateway, rewrite_to)?;
        config.save(config_path)?;
        println!("Rule edited successfully.");
        notify_live_apply();
    }
    Ok(())
}

fn handle_start_command() -> Result<()> {
    if !service::is_installed()? {
        return Err(anyhow!(
            "The packet router runs as a Windows service. Install it first (elevated PowerShell), e.g. re-run installer.ps1 or:\n  \
             roust --install-service\nThen start it:\n  roust start"
        ));
    }
    service::start()
}

fn handle_stop_command() -> Result<()> {
    service::stop()
}

fn handle_restart_command() -> Result<()> {
    service::restart()
}

fn handle_status_command() -> Result<()> {
    service::print_status()
}
