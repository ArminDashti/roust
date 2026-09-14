//! Management HTTP API for service status, routes, and per-app NIC binds.

use crate::config::{
    apply_hosts, apply_nrpt, AppBind, AppBindStatus, AppBindStore, Config, DnsException,
    DnsExceptionStore, HostOverride, HostOverrideStore, RoutingRule,
};
use crate::network::{
    discover_external_routes, enumerate_interfaces, list_processes, ping_via_nic, ProcessItem,
    PingResult,
};
use crate::service;
use crate::wfp::resolve_bind_status;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use windows_service::service::ServiceState;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_BIND: &str = "127.0.0.1:8787";

#[derive(Clone)]
pub struct AppState {
    pub config_path: PathBuf,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub installed: bool,
    pub state: String,
    pub config_path: String,
    pub rule_count: usize,
    pub config_rule_count: usize,
    pub system_rule_count: usize,
    pub version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteSource {
    Config,
    System,
}

#[derive(Serialize)]
pub struct RouteItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    #[serde(flatten)]
    pub rule: RoutingRule,
    pub source: RouteSource,
}

#[derive(Serialize)]
pub struct ErrorBody {
    pub error: String,
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

fn service_state_label(state: ServiceState) -> &'static str {
    match state {
        ServiceState::Stopped => "Stopped",
        ServiceState::StartPending => "StartPending",
        ServiceState::StopPending => "StopPending",
        ServiceState::Running => "Running",
        ServiceState::ContinuePending => "ContinuePending",
        ServiceState::PausePending => "PausePending",
        ServiceState::Paused => "Paused",
    }
}

fn load_config(path: &PathBuf) -> Result<Config, ApiError> {
    if !path.exists() {
        return Ok(Config::new());
    }
    Config::load(path).map_err(|e| ApiError::bad_request(e.to_string()))
}

fn save_config(path: &PathBuf, config: &Config) -> Result<(), ApiError> {
    config
        .save(path)
        .map_err(|e| ApiError::internal(format!("failed to save routes: {e}")))
}

fn app_binds_path(routes_path: &PathBuf) -> PathBuf {
    AppBindStore::path_beside(routes_path)
}

fn load_app_binds(routes_path: &PathBuf) -> Result<AppBindStore, ApiError> {
    AppBindStore::load(app_binds_path(routes_path))
        .map_err(|e| ApiError::bad_request(e.to_string()))
}

fn save_app_binds(routes_path: &PathBuf, store: &AppBindStore) -> Result<(), ApiError> {
    store
        .save(app_binds_path(routes_path))
        .map_err(|e| ApiError::internal(format!("failed to save app-binds: {e}")))
}

fn dns_exceptions_path(routes_path: &PathBuf) -> PathBuf {
    DnsExceptionStore::path_beside(routes_path)
}

fn load_dns_exceptions(routes_path: &PathBuf) -> Result<DnsExceptionStore, ApiError> {
    DnsExceptionStore::load(dns_exceptions_path(routes_path))
        .map_err(|e| ApiError::bad_request(e.to_string()))
}

fn save_dns_exceptions(routes_path: &PathBuf, store: &DnsExceptionStore) -> Result<(), ApiError> {
    store
        .save(dns_exceptions_path(routes_path))
        .map_err(|e| ApiError::internal(format!("failed to save dns-exceptions: {e}")))
}

fn apply_dns_exceptions(store: &DnsExceptionStore) -> Result<(), ApiError> {
    apply_nrpt(store.get_exceptions())
        .map_err(|e| ApiError::internal(format!("failed to apply NRPT dns exceptions: {e}")))
}

fn host_overrides_path(routes_path: &PathBuf) -> PathBuf {
    HostOverrideStore::path_beside(routes_path)
}

fn load_host_overrides(routes_path: &PathBuf) -> Result<HostOverrideStore, ApiError> {
    HostOverrideStore::load(host_overrides_path(routes_path))
        .map_err(|e| ApiError::bad_request(e.to_string()))
}

fn save_host_overrides(routes_path: &PathBuf, store: &HostOverrideStore) -> Result<(), ApiError> {
    store
        .save(host_overrides_path(routes_path))
        .map_err(|e| ApiError::internal(format!("failed to save host-overrides: {e}")))
}

fn apply_host_overrides(store: &HostOverrideStore) -> Result<(), ApiError> {
    apply_hosts(store.get_overrides())
        .map_err(|e| ApiError::internal(format!("failed to apply host overrides: {e}")))
}

fn maybe_restart_running_service() -> Result<(), ApiError> {
    let installed = service::is_installed().unwrap_or(false);
    if !installed {
        return Ok(());
    }
    let state = service::query_state().map_err(|e| ApiError::internal(e.to_string()))?;
    if matches!(state, ServiceState::Running) {
        service::restart().map_err(|e| ApiError::internal(e.to_string()))?;
    }
    Ok(())
}

async fn get_status(State(state): State<Arc<AppState>>) -> Result<Json<StatusResponse>, ApiError> {
    let installed = service::is_installed().unwrap_or(false);
    let state_label = if installed {
        match service::query_state() {
            Ok(s) => service_state_label(s).to_string(),
            Err(e) => format!("Error: {e}"),
        }
    } else {
        "NotInstalled".to_string()
    };

    let config = load_config(&state.config_path)?;
    let config_rules = config.get_rules();
    let system_rules = discover_external_routes(config_rules).unwrap_or_default();
    Ok(Json(StatusResponse {
        installed,
        state: state_label,
        config_path: state.config_path.display().to_string(),
        rule_count: config_rules.len() + system_rules.len(),
        config_rule_count: config_rules.len(),
        system_rule_count: system_rules.len(),
        version: VERSION.to_string(),
    }))
}

async fn list_routes(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<RouteItem>>, ApiError> {
    let config = load_config(&state.config_path)?;
    let config_rules = config.get_rules();
    let system_rules = discover_external_routes(config_rules).map_err(|e| {
        ApiError::internal(format!("failed to read applied host routes: {e}"))
    })?;

    let mut items = config_rules
        .iter()
        .enumerate()
        .map(|(index, rule)| RouteItem {
            index: Some(index),
            rule: rule.clone(),
            source: RouteSource::Config,
        })
        .collect::<Vec<_>>();

    for rule in system_rules {
        items.push(RouteItem {
            index: None,
            rule,
            source: RouteSource::System,
        });
    }

    Ok(Json(items))
}

async fn create_route(
    State(state): State<Arc<AppState>>,
    Json(rule): Json<RoutingRule>,
) -> Result<(StatusCode, Json<RouteItem>), ApiError> {
    let mut config = load_config(&state.config_path)?;
    config
        .add_rule(rule.clone())
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    save_config(&state.config_path, &config)?;
    maybe_restart_running_service()?;
    let index = config.get_rules().len().saturating_sub(1);
    Ok((
        StatusCode::CREATED,
        Json(RouteItem {
            index: Some(index),
            rule,
            source: RouteSource::Config,
        }),
    ))
}

async fn update_route(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
    Json(rule): Json<RoutingRule>,
) -> Result<Json<RouteItem>, ApiError> {
    let mut config = load_config(&state.config_path)?;
    if index >= config.get_rules().len() {
        return Err(ApiError::not_found(format!("rule index {index} not found")));
    }
    config
        .replace_rule_at(index, rule.clone())
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    save_config(&state.config_path, &config)?;
    maybe_restart_running_service()?;
    Ok(Json(RouteItem {
        index: Some(index),
        rule,
        source: RouteSource::Config,
    }))
}

async fn delete_route(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
) -> Result<StatusCode, ApiError> {
    let mut config = load_config(&state.config_path)?;
    if !config.remove_rule_at(index) {
        return Err(ApiError::not_found(format!("rule index {index} not found")));
    }
    save_config(&state.config_path, &config)?;
    maybe_restart_running_service()?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct BatchRoutesRequest {
    rules: Vec<RoutingRule>,
}

async fn create_routes_batch(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BatchRoutesRequest>,
) -> Result<(StatusCode, Json<Vec<RouteItem>>), ApiError> {
    if body.rules.is_empty() {
        return Err(ApiError::bad_request("rules must not be empty"));
    }

    let mut config = load_config(&state.config_path)?;
    let start_index = config.get_rules().len();
    for rule in &body.rules {
        config
            .add_rule(rule.clone())
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
    }
    save_config(&state.config_path, &config)?;
    maybe_restart_running_service()?;

    let items = body
        .rules
        .into_iter()
        .enumerate()
        .map(|(offset, rule)| RouteItem {
            index: Some(start_index + offset),
            rule,
            source: RouteSource::Config,
        })
        .collect();

    Ok((StatusCode::CREATED, Json(items)))
}

#[derive(Serialize)]
struct AdapterItem {
    name: String,
    display_name: String,
    friendly_name: Option<String>,
    mac_address: String,
    if_index: u32,
    ipv4_address: Option<String>,
    status: String,
}

async fn list_adapters() -> Result<Json<Vec<AdapterItem>>, ApiError> {
    let interfaces = enumerate_interfaces()
        .map_err(|e| ApiError::internal(format!("failed to enumerate adapters: {e}")))?;
    let items = interfaces
        .into_iter()
        .map(|iface| AdapterItem {
            name: iface.name,
            display_name: iface.display_name,
            friendly_name: iface.friendly_name,
            mac_address: iface.mac_address,
            if_index: iface.if_index,
            ipv4_address: iface.ipv4_address,
            status: iface.status,
        })
        .collect();
    Ok(Json(items))
}

async fn list_processes_handler() -> Result<Json<Vec<ProcessItem>>, ApiError> {
    let items = tokio::task::spawn_blocking(list_processes)
        .await
        .map_err(|e| ApiError::internal(format!("process list task failed: {e}")))?
        .map_err(|e| ApiError::internal(format!("failed to list processes: {e}")))?;
    Ok(Json(items))
}

#[derive(Serialize)]
struct AppBindItem {
    index: usize,
    #[serde(flatten)]
    bind: AppBind,
    status: AppBindStatus,
}

async fn list_app_binds(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AppBindItem>>, ApiError> {
    let store = load_app_binds(&state.config_path)?;
    let interfaces = enumerate_interfaces()
        .map_err(|e| ApiError::internal(format!("failed to enumerate adapters: {e}")))?;
    let items = store
        .get_binds()
        .iter()
        .enumerate()
        .map(|(index, bind)| AppBindItem {
            index,
            bind: bind.clone(),
            status: resolve_bind_status(bind, &interfaces),
        })
        .collect();
    Ok(Json(items))
}

async fn create_app_bind(
    State(state): State<Arc<AppState>>,
    Json(bind): Json<AppBind>,
) -> Result<(StatusCode, Json<AppBindItem>), ApiError> {
    let mut store = load_app_binds(&state.config_path)?;
    store
        .add(bind)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    save_app_binds(&state.config_path, &store)?;
    maybe_restart_running_service()?;
    let index = store.get_binds().len().saturating_sub(1);
    let bind = store.get_binds()[index].clone();
    let interfaces = enumerate_interfaces().unwrap_or_default();
    Ok((
        StatusCode::CREATED,
        Json(AppBindItem {
            index,
            status: resolve_bind_status(&bind, &interfaces),
            bind,
        }),
    ))
}

async fn update_app_bind(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
    Json(bind): Json<AppBind>,
) -> Result<Json<AppBindItem>, ApiError> {
    let mut store = load_app_binds(&state.config_path)?;
    store
        .replace_at(index, bind)
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::not_found(e.to_string())
            } else {
                ApiError::bad_request(e.to_string())
            }
        })?;
    save_app_binds(&state.config_path, &store)?;
    maybe_restart_running_service()?;
    let bind = store.get_binds()[index].clone();
    let interfaces = enumerate_interfaces().unwrap_or_default();
    Ok(Json(AppBindItem {
        index,
        status: resolve_bind_status(&bind, &interfaces),
        bind,
    }))
}

async fn delete_app_bind(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
) -> Result<StatusCode, ApiError> {
    let mut store = load_app_binds(&state.config_path)?;
    if !store.remove_at(index) {
        return Err(ApiError::not_found(format!(
            "app bind index {index} not found"
        )));
    }
    save_app_binds(&state.config_path, &store)?;
    maybe_restart_running_service()?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct DnsExceptionItem {
    index: usize,
    #[serde(flatten)]
    exception: DnsException,
}

async fn list_dns_exceptions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DnsExceptionItem>>, ApiError> {
    let store = load_dns_exceptions(&state.config_path)?;
    let items = store
        .get_exceptions()
        .iter()
        .enumerate()
        .map(|(index, exception)| DnsExceptionItem {
            index,
            exception: exception.clone(),
        })
        .collect();
    Ok(Json(items))
}

async fn create_dns_exception(
    State(state): State<Arc<AppState>>,
    Json(exception): Json<DnsException>,
) -> Result<(StatusCode, Json<DnsExceptionItem>), ApiError> {
    let mut store = load_dns_exceptions(&state.config_path)?;
    store
        .add(exception)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    save_dns_exceptions(&state.config_path, &store)?;
    apply_dns_exceptions(&store)?;
    let index = store.get_exceptions().len().saturating_sub(1);
    let exception = store.get_exceptions()[index].clone();
    Ok((
        StatusCode::CREATED,
        Json(DnsExceptionItem { index, exception }),
    ))
}

async fn update_dns_exception(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
    Json(exception): Json<DnsException>,
) -> Result<Json<DnsExceptionItem>, ApiError> {
    let mut store = load_dns_exceptions(&state.config_path)?;
    store.replace_at(index, exception).map_err(|e| {
        if e.to_string().contains("not found") {
            ApiError::not_found(e.to_string())
        } else {
            ApiError::bad_request(e.to_string())
        }
    })?;
    save_dns_exceptions(&state.config_path, &store)?;
    apply_dns_exceptions(&store)?;
    let exception = store.get_exceptions()[index].clone();
    Ok(Json(DnsExceptionItem { index, exception }))
}

async fn delete_dns_exception(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
) -> Result<StatusCode, ApiError> {
    let mut store = load_dns_exceptions(&state.config_path)?;
    if !store.remove_at(index) {
        return Err(ApiError::not_found(format!(
            "dns exception index {index} not found"
        )));
    }
    save_dns_exceptions(&state.config_path, &store)?;
    apply_dns_exceptions(&store)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct HostOverrideItem {
    index: usize,
    #[serde(flatten)]
    override_: HostOverride,
}

async fn list_host_overrides(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<HostOverrideItem>>, ApiError> {
    let store = load_host_overrides(&state.config_path)?;
    let items = store
        .get_overrides()
        .iter()
        .enumerate()
        .map(|(index, override_)| HostOverrideItem {
            index,
            override_: override_.clone(),
        })
        .collect();
    Ok(Json(items))
}

async fn create_host_override(
    State(state): State<Arc<AppState>>,
    Json(override_): Json<HostOverride>,
) -> Result<(StatusCode, Json<HostOverrideItem>), ApiError> {
    let mut store = load_host_overrides(&state.config_path)?;
    store
        .add(override_)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    // Apply before save so a locked hosts file does not leave orphan JSON.
    apply_host_overrides(&store)?;
    save_host_overrides(&state.config_path, &store)?;
    let index = store.get_overrides().len().saturating_sub(1);
    let override_ = store.get_overrides()[index].clone();
    Ok((
        StatusCode::CREATED,
        Json(HostOverrideItem { index, override_ }),
    ))
}

async fn update_host_override(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
    Json(override_): Json<HostOverride>,
) -> Result<Json<HostOverrideItem>, ApiError> {
    let mut store = load_host_overrides(&state.config_path)?;
    store.replace_at(index, override_).map_err(|e| {
        if e.to_string().contains("not found") {
            ApiError::not_found(e.to_string())
        } else {
            ApiError::bad_request(e.to_string())
        }
    })?;
    apply_host_overrides(&store)?;
    save_host_overrides(&state.config_path, &store)?;
    let override_ = store.get_overrides()[index].clone();
    Ok(Json(HostOverrideItem { index, override_ }))
}

async fn delete_host_override(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
) -> Result<StatusCode, ApiError> {
    let mut store = load_host_overrides(&state.config_path)?;
    if !store.remove_at(index) {
        return Err(ApiError::not_found(format!(
            "host override index {index} not found"
        )));
    }
    apply_host_overrides(&store)?;
    save_host_overrides(&state.config_path, &store)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct PingRequest {
    host: String,
    nic: String,
    #[serde(default)]
    count: Option<u32>,
}

async fn post_ping(Json(body): Json<PingRequest>) -> Result<Json<PingResult>, ApiError> {
    let host = body.host.trim().to_string();
    let nic = body.nic.trim().to_string();
    if host.is_empty() {
        return Err(ApiError::bad_request("host must not be empty"));
    }
    if nic.is_empty() {
        return Err(ApiError::bad_request("nic must not be empty"));
    }

    let result = tokio::task::spawn_blocking(move || ping_via_nic(&host, &nic, body.count))
        .await
        .map_err(|e| ApiError::internal(format!("ping task failed: {e}")))?
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    Ok(Json(result))
}

#[derive(Serialize)]
struct ServiceActionResponse {
    ok: bool,
    installed: bool,
    state: String,
}

fn service_action_response() -> Result<Json<ServiceActionResponse>, ApiError> {
    let installed = service::is_installed().unwrap_or(false);
    let state = if installed {
        match service::query_state() {
            Ok(s) => service_state_label(s).to_string(),
            Err(e) => format!("Error: {e}"),
        }
    } else {
        "NotInstalled".to_string()
    };
    Ok(Json(ServiceActionResponse {
        ok: true,
        installed,
        state,
    }))
}

async fn service_install() -> Result<Json<ServiceActionResponse>, ApiError> {
    service::install(true).map_err(|e| ApiError::internal(format!("{e:#}")))?;
    service_action_response()
}

async fn service_start() -> Result<Json<ServiceActionResponse>, ApiError> {
    service::start().map_err(|e| ApiError::internal(format!("{e:#}")))?;
    service_action_response()
}

async fn service_stop() -> Result<Json<ServiceActionResponse>, ApiError> {
    service::stop().map_err(|e| ApiError::internal(format!("{e:#}")))?;
    service_action_response()
}

async fn service_restart() -> Result<Json<ServiceActionResponse>, ApiError> {
    service::restart().map_err(|e| ApiError::internal(format!("{e:#}")))?;
    service_action_response()
}

fn cors_layer() -> CorsLayer {
    let origins = [
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://localhost:4173",
        "http://127.0.0.1:4173",
        "http://pc-armin",
        "http://pc-armin:80",
    ]
    .into_iter()
    .filter_map(|o| o.parse::<HeaderValue>().ok())
    .collect::<Vec<_>>();

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(tower_http::cors::Any)
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/adapters", get(list_adapters))
        .route("/api/processes", get(list_processes_handler))
        .route("/api/ping", post(post_ping))
        .route("/api/routes", get(list_routes).post(create_route))
        .route("/api/routes/batch", post(create_routes_batch))
        .route(
            "/api/routes/{index}",
            axum::routing::put(update_route).delete(delete_route),
        )
        .route("/api/app-binds", get(list_app_binds).post(create_app_bind))
        .route(
            "/api/app-binds/{index}",
            axum::routing::put(update_app_bind).delete(delete_app_bind),
        )
        .route(
            "/api/dns-exceptions",
            get(list_dns_exceptions).post(create_dns_exception),
        )
        .route(
            "/api/dns-exceptions/{index}",
            axum::routing::put(update_dns_exception).delete(delete_dns_exception),
        )
        .route(
            "/api/host-overrides",
            get(list_host_overrides).post(create_host_override),
        )
        .route(
            "/api/host-overrides/{index}",
            axum::routing::put(update_host_override).delete(delete_host_override),
        )
        .route("/api/service/install", post(service_install))
        .route("/api/service/start", post(service_start))
        .route("/api/service/stop", post(service_stop))
        .route("/api/service/restart", post(service_restart))
        .layer(cors_layer())
        .with_state(state)
}

#[derive(Debug, Deserialize)]
pub struct ApiOptions {
    pub bind: String,
    pub config_path: Option<PathBuf>,
}

impl Default for ApiOptions {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND.to_string(),
            config_path: None,
        }
    }
}

pub async fn serve(options: ApiOptions) -> anyhow::Result<()> {
    let config_path = options
        .config_path
        .unwrap_or_else(Config::default_config_path);
    let state = Arc::new(AppState { config_path });
    let app = router(state.clone());
    let addr: SocketAddr = options
        .bind
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid --bind address '{}': {e}", options.bind))?;

    log::info!(
        "roust-api listening on http://{addr} (config: {})",
        state.config_path.display()
    );
    println!(
        "roust-api listening on http://{addr} (config: {})",
        state.config_path.display()
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    log::info!("roust-api shutting down");
}

