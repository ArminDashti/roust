mod cli;
mod config;
mod core;
mod network;
mod service;

use anyhow::{anyhow, Context, Result};
use cli::{parse_cli, Commands, NicCommands, RouteCommands, RuleAction, ServiceCommands};
use config::Config;
use network::enumerate_interfaces;
use roust::update;
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
    let config_path = cli.config.unwrap_or_else(Config::default_config_path);
    match cli.command {
        Commands::Nics { action } => handle_nics_command(action)?,
        Commands::Route { action } => handle_route_command(action)?,
        Commands::Add { action } => handle_add_rule(action, &config_path)?,
        Commands::Delete { action } => handle_delete_rule(action, &config_path)?,
        Commands::Edit { action } => handle_edit_rule(action, &config_path)?,
        Commands::Start => handle_start_command()?,
        Commands::Stop => handle_stop_command()?,
        Commands::Restart => handle_restart_command()?,
        Commands::Status => handle_status_command()?,
        Commands::Service { action } => handle_service_command(action)?,
        Commands::Update => {
            let out_dir =
                env::current_dir().context("resolve current directory for roust update")?;
            update::run(&out_dir)?;
            println!(
                "Updated ipv4.txt, ipv6.txt, ipv4_cidr.txt, ipv6_cidr.txt in {}",
                out_dir.display()
            );
        }
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

fn handle_route_command(action: RouteCommands) -> Result<()> {
    match action {
        RouteCommands::Predict { dest } => {
            let ip: Ipv4Addr = dest
                .parse()
                .map_err(|_| anyhow!("--dest must be a valid IPv4 address (e.g. 8.8.8.8)"))?;
            let p = network::predict_ipv4_egress(ip)?;
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
        }
    }
    Ok(())
}

fn handle_nics_command(action: NicCommands) -> Result<()> {
    match action {
        NicCommands::List => {
            println!(
                "\n{:<20} {:<40} {:<20} {:<15} {:<20}",
                "Name", "Description", "MAC Address", "Type", "IPv4 Address"
            );
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
    let RuleAction::Rule {
        ip,
        nic,
        file,
        rewrite_to,
    } = action;
    let mut config = Config::load(config_path).unwrap_or_else(|_| Config::new());
    let interfaces = enumerate_interfaces()?;
    let validate_nic = |nic_name: &str| -> Result<()> {
        if interfaces
            .iter()
            .any(|n| network::nic_name_matches(n, nic_name))
        {
            Ok(())
        } else {
            Err(anyhow!("Interface '{}' not found", nic_name))
        }
    };

    if let Some(file_path) = file {
        let content = fs::read_to_string(&file_path)?;
        let imported = Config::parse_import_file(&content, &file_path)?;
        for rule in imported {
            let dest = if rule.nic.is_empty() {
                nic.as_ref()
                    .ok_or_else(|| {
                        anyhow!(
                            "Each entry in {} needs a \"nic\", or pass --nic for IP-only lists",
                            file_path.display()
                        )
                    })?
                    .clone()
            } else {
                rule.nic
            };
            validate_nic(&dest)?;
            let rule_rewrite = rule.rewrite_to.or_else(|| rewrite_to.clone());
            config.add_rule(rule.ip, dest, rule_rewrite)?;
        }
    } else if let (Some(ip_addr), Some(dest)) = (ip, nic) {
        validate_nic(&dest)?;
        config.add_rule(ip_addr, dest, rewrite_to)?;
    } else {
        return Err(anyhow!(
            "Provide --ip and --nic, or --file (e.g. routes.json with ip and nic per entry)"
        ));
    }
    config.save(config_path)?;
    println!("Rule(s) added successfully.");
    Ok(())
}

fn handle_delete_rule(action: RuleAction, config_path: &PathBuf) -> Result<()> {
    let RuleAction::Rule { ip, .. } = action;
    if let Some(ip_addr) = ip {
        let mut config = Config::load(config_path)?;
        if config.remove_rule(&ip_addr) {
            config.save(config_path)?;
            println!("Rule deleted successfully.");
        } else {
            println!("Rule not found.");
        }
    }
    Ok(())
}

fn handle_edit_rule(action: RuleAction, config_path: &PathBuf) -> Result<()> {
    let RuleAction::Rule {
        ip,
        nic,
        rewrite_to,
        ..
    } = action;
    if let (Some(ip_addr), Some(new_nic)) = (ip, nic) {
        let mut config = Config::load(config_path)?;
        config.remove_rule(&ip_addr);
        config.add_rule(ip_addr, new_nic, rewrite_to)?;
        config.save(config_path)?;
        println!("Rule edited successfully.");
    }
    Ok(())
}

fn handle_start_command() -> Result<()> {
    if !service::is_installed()? {
        return Err(anyhow!(
            "The packet router runs as a Windows service. Install it first (elevated PowerShell):\n  \
             roust service install\nThen start it:\n  roust start"
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

fn handle_service_command(action: ServiceCommands) -> Result<()> {
    match action {
        ServiceCommands::Install { auto } => service::install(auto),
        ServiceCommands::Uninstall => service::uninstall(),
    }
}
