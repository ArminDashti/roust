use roust::api;
use roust::config::RoutingRule;
use std::path::{Path, PathBuf};

fn config_path() -> PathBuf {
    api::resolve_config_path(None)
}

fn map_err<T, E: std::fmt::Display>(result: std::result::Result<T, E>) -> Result<T, String> {
    result.map_err(|e| e.to_string())
}

#[tauri::command]
fn get_config_path() -> String {
    config_path().display().to_string()
}

#[tauri::command]
fn get_status() -> Result<api::ServiceStatus, String> {
    map_err(api::service_status())
}

#[tauri::command]
fn list_rules() -> Result<Vec<RoutingRule>, String> {
    map_err(api::list_rules(&config_path()))
}

#[tauri::command]
fn list_gateways() -> Result<Vec<api::GatewayRow>, String> {
    map_err(api::list_gateways())
}

#[tauri::command]
fn predict_ip(ip: String) -> Result<api::PredictResult, String> {
    map_err(api::predict_route(&ip))
}

#[tauri::command]
fn add_rule(cidr: String, rewrite_to: String) -> Result<api::RuleMutationResult, String> {
    map_err(api::add_rule(&config_path(), cidr, rewrite_to))
}

#[tauri::command]
fn delete_rule(cidr: String) -> Result<api::RuleMutationResult, String> {
    map_err(api::delete_rule(&config_path(), &cidr))
}

#[tauri::command]
fn edit_rule(cidr: String, rewrite_to: String) -> Result<api::RuleMutationResult, String> {
    map_err(api::edit_rule(&config_path(), cidr, rewrite_to))
}

#[tauri::command]
fn import_rules(
    file_path: String,
    default_rewrite_to: Option<String>,
) -> Result<api::RuleMutationResult, String> {
    map_err(api::import_rules_from_file(
        &config_path(),
        Path::new(&file_path),
        default_rewrite_to,
    ))
}

#[tauri::command]
fn start_service() -> Result<String, String> {
    map_err(api::start_service())
}

#[tauri::command]
fn stop_service() -> Result<String, String> {
    map_err(api::stop_service())
}

#[tauri::command]
fn restart_service() -> Result<String, String> {
    map_err(api::restart_service())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_config_path,
            get_status,
            list_rules,
            list_gateways,
            predict_ip,
            add_rule,
            delete_rule,
            edit_rule,
            import_rules,
            start_service,
            stop_service,
            restart_service,
        ])
        .run(tauri::generate_context!())
        .expect("error while running roust GUI");
}
