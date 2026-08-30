//! Management HTTP API for service status and route CRUD.

use crate::config::{Config, RoutingRule};
use crate::network::{discover_external_routes, enumerate_interfaces};
use crate::service;
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
        })
        .collect();
    Ok(Json(items))
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
    service::install(true).map_err(|e| ApiError::internal(e.to_string()))?;
    service_action_response()
}

async fn service_start() -> Result<Json<ServiceActionResponse>, ApiError> {
    service::start().map_err(|e| ApiError::internal(e.to_string()))?;
    service_action_response()
}

async fn service_stop() -> Result<Json<ServiceActionResponse>, ApiError> {
    service::stop().map_err(|e| ApiError::internal(e.to_string()))?;
    service_action_response()
}

async fn service_restart() -> Result<Json<ServiceActionResponse>, ApiError> {
    service::restart().map_err(|e| ApiError::internal(e.to_string()))?;
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
        .route("/api/routes", get(list_routes).post(create_route))
        .route("/api/routes/batch", post(create_routes_batch))
        .route(
            "/api/routes/{index}",
            axum::routing::put(update_route).delete(delete_route),
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

