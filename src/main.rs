mod cli;
mod config;
mod nics;

use anyhow::Result;
use cli::{parse_cli, Commands, NicCommands, IpCommands, IpDestCommands};
use config::Config;
use nics::enumerate_interfaces;
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
        Commands::Ip { action } => handle_ip_command(action, &config_path)?,
        Commands::Start { service, install_service } => {
            handle_start_command(service, install_service)?;
        }
        Commands::Stop { service, uninstall_service } => {
            handle_stop_command(service, uninstall_service)?;
        }
    }

    Ok(())
}

fn handle_nics_command(action: NicCommands) -> Result<()> {
    match action {
        NicCommands::Show => {
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

            if interfaces.is_empty() {
                println!("No network interfaces found.");
            }
            Ok(())
        }
    }
}

fn handle_ip_command(action: IpCommands, config_path: &PathBuf) -> Result<()> {
    match action {
        IpCommands::Dest { action } => match action {
            IpDestCommands::Show { ip } => {
                // Load config
                let config = load_config(config_path)?;

                // Find destination
                match config.find_destination(&ip) {
                    Some(dest) => {
                        println!("IP {} -> NIC: {}", ip, dest);
                    }
                    None => {
                        println!("No routing rule found for IP: {}", ip);
                    }
                }

                Ok(())
            }

            IpDestCommands::Add { ip, dest, file } => {
                // Load existing config or create new
                let mut config = load_config(config_path).unwrap_or_else(|_| Config::new());

                // Validate NIC exists
                let nics = enumerate_interfaces()?;
                if !nics.iter().any(|nic| nic.name.eq_ignore_ascii_case(&dest)) {
                    eprintln!("Error: Network interface '{}' not found", dest);
                    println!("Available NICs:");
                    for nic in nics {
                        println!("  - {} ({})", nic.name, nic.display_name);
                    }
                    return Err(anyhow::anyhow!("Invalid NIC: {}", dest));
                }

                // Handle file input or direct IP
                if let Some(file_path) = file {
                    // Load IPs from file
                    let file_contents = fs::read_to_string(&file_path)?;
                    let mut added_count = 0;

                    for line in file_contents.lines() {
                        let ip_str = line.trim();
                        if !ip_str.is_empty() && !ip_str.starts_with('#') {
                            config.add_rule(ip_str.to_string(), dest.clone())?;
                            added_count += 1;
                        }
                    }

                    println!("Added {} routes from file: {:?}", added_count, file_path);
                } else if let Some(ip_str) = ip {
                    // Add single IP
                    config.add_rule(ip_str.clone(), dest.clone())?;
                    println!("Added route: {} -> {}", ip_str, dest);
                } else {
                    return Err(anyhow::anyhow!(
                        "Either --ip or --file must be provided"
                    ));
                }

                // Save config
                config.save(config_path)?;
                println!("Config saved to {:?}", config_path);

                Ok(())
            }
        },
    }
}

fn handle_start_command(service: bool, install_service: bool) -> Result<()> {
    if service || install_service {
        println!("[TODO] Windows Service start not yet implemented");
        println!("Service mode requested (install_service={})", install_service);
    } else {
        println!("[TODO] Daemon mode start not yet implemented");
        println!("Would start router in daemon mode...");
    }
    Ok(())
}

fn handle_stop_command(service: bool, uninstall_service: bool) -> Result<()> {
    if service || uninstall_service {
        println!("[TODO] Windows Service stop not yet implemented");
        println!("Service mode requested (uninstall_service={})", uninstall_service);
    } else {
        println!("[TODO] Daemon mode stop not yet implemented");
        println!("Would stop router daemon...");
    }
    Ok(())
}

fn load_config(config_path: &PathBuf) -> Result<Config> {
    if config_path.exists() {
        Config::load(config_path)
    } else {
        Err(anyhow::anyhow!(
            "Config file not found: {:?}. Create one with 'roust ip dest add' command.",
            config_path
        ))
    }
}
