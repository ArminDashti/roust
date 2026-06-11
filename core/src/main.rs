use anyhow::{anyhow, Context, Result};

use roust::api;

use roust::cli::{parse_cli, Commands, GatewayCommands, RuleAction, RuleCommands};

use roust::service;

use std::env;

use std::fs::OpenOptions;

use std::io::Write;

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



    let config_path = api::resolve_config_path(cli.config);

    let command = cli

        .command

        .ok_or_else(|| anyhow!("missing subcommand (run roust --help)"))?;



    match command {

        Commands::Predict { ip } => handle_predict_command(&ip)?,

        Commands::Rule { action } => handle_rule_command(action, &config_path)?,

        Commands::Gateway { action } => handle_gateway_command(action)?,

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

    let p = api::predict_route(ip)?;

    println!("destination:  {}", p.destination);

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



fn handle_gateway_command(action: GatewayCommands) -> Result<()> {

    match action {

        GatewayCommands::List => {

            let rows = api::list_gateways()?;

            if rows.is_empty() {

                println!("No default gateways found on local interfaces.");

                return Ok(());

            }

            println!("{:<32} {:<20} {}", "nic-name", "mac", "gateway-ip");

            for row in rows {

                println!("{:<32} {:<20} {}", row.nic_name, row.mac, row.gateway_ip);

            }

        }

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

            let rules = api::list_rules(config_path)?;

            if rules.is_empty() {

                println!("No routing rules in {}.", config_path.display());

                return Ok(());

            }

            println!("Rules from {} ({}):", config_path.display(), rules.len());

            println!(

                "\n{:<24} {:<20} {:<18} {:<18} {:<18}",

                "IP / CIDR", "MAC", "NIC", "Gateway", "rewrite_to"

            );

            println!("{}", "-".repeat(102));

            for rule in rules {

                let mac = rule.mac.as_deref().unwrap_or("-");

                let nic = rule.nic.as_deref().unwrap_or("-");

                let gw = rule.gateway.as_deref().unwrap_or("-");

                let rewrite = rule.rewrite_to.as_deref().unwrap_or("-");

                println!(

                    "{:<24} {:<20} {:<18} {:<18} {:<18}",

                    rule.ip, mac, nic, gw, rewrite

                );

            }

        }

    }

    Ok(())

}



fn print_mutation_result(result: api::RuleMutationResult) {

    println!("{}", result.message);

    if let Some(hint) = result.live_apply_hint {

        println!("{hint}");

    }

}



fn handle_add_rule(action: RuleAction, config_path: &PathBuf) -> Result<()> {

    let RuleAction::Rule {

        ip,

        mac,

        nic,

        gateway,

        file,

        rewrite_to,

    } = action;



    if let Some(file_path) = file {

        let result = api::import_rules_from_file(config_path, &file_path, mac, nic, gateway, rewrite_to)?;

        print_mutation_result(result);

    } else if let Some(ip_addr) = ip {

        let result = api::add_rule(config_path, ip_addr, mac, nic, gateway, rewrite_to)?;

        print_mutation_result(result);

    } else {

        return Err(anyhow!(

            "Provide --ip with at least one of --mac, --nic, --gateway, or --file. \
             Run `roust gateway list` to see adapters."

        ));

    }

    Ok(())

}



fn handle_delete_rule(action: RuleAction, config_path: &PathBuf) -> Result<()> {

    let RuleAction::Rule { ip, .. } = action;

    if let Some(ip_addr) = ip {

        let result = api::delete_rule(config_path, &ip_addr)?;

        print_mutation_result(result);

    }

    Ok(())

}



fn handle_edit_rule(action: RuleAction, config_path: &PathBuf) -> Result<()> {

    let RuleAction::Rule {

        ip,

        mac,

        nic,

        gateway,

        rewrite_to,

        ..

    } = action;

    if let Some(ip_addr) = ip {

        let result = api::edit_rule(config_path, ip_addr, mac, nic, gateway, rewrite_to)?;

        print_mutation_result(result);

    }

    Ok(())

}



fn handle_start_command() -> Result<()> {

    api::start_service()?;

    Ok(())

}



fn handle_stop_command() -> Result<()> {

    api::stop_service()?;

    Ok(())

}



fn handle_restart_command() -> Result<()> {

    api::restart_service()?;

    Ok(())

}



fn handle_status_command() -> Result<()> {

    let status = api::service_status()?;

    let state_label = match status.state.as_str() {

        "started" => "started",

        "stopped" => "stopped",

        _ => "stopped",

    };

    println!("{state_label} roust");

    println!("directory: {}", status.directory);

    println!("version: {}", status.version);

    Ok(())

}


