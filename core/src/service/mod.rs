//! Windows Service Control Manager (SCM) integration for the packet router daemon.

use crate::config::Config;
use crate::core::{self, PacketRouter};
use anyhow::{anyhow, Context, Result};
use std::env;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use windows_service::define_windows_service;
use windows_service::service::{
    ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceInfo,
    ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

pub const SERVICE_NAME: &str = "Roust";
pub const SERVICE_DISPLAY_NAME: &str = "roust Packet Router";
/// SCM invokes the binary with this flag (must match `service_binary_arguments`).
pub const RUN_AS_SERVICE_FLAG: &str = "--run-as-service";

struct FileLogTarget(Mutex<std::fs::File>);

impl Write for FileLogTarget {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

/// Directory containing `roust.exe` (install root for WinDivert DLL and default config).
pub fn exe_install_dir() -> Result<PathBuf> {
    let exe = env::current_exe().context("resolve path to roust.exe")?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("roust.exe has no parent directory"))
}

/// Ensure the process cwd is the install directory so relative paths resolve consistently.
pub fn set_working_directory_to_install_dir() -> Result<()> {
    let dir = exe_install_dir()?;
    env::set_current_dir(&dir).with_context(|| format!("set working directory to {}", dir.display()))
}

fn service_log_path() -> Result<PathBuf> {
    let dir = exe_install_dir()?;
    let logs = dir.join("logs");
    std::fs::create_dir_all(&logs)
        .with_context(|| format!("create logs directory {}", logs.display()))?;
    Ok(logs.join("roust-service.log"))
}

fn init_service_file_logger() -> Result<()> {
    let log_path = service_log_path()?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("open service log {}", log_path.display()))?;
    env_logger::Builder::from_default_env()
        .format_timestamp_secs()
        .target(env_logger::Target::Pipe(Box::new(FileLogTarget(Mutex::new(file)))))
        .init();
    log::info!("Service logging to {}", log_path.display());
    Ok(())
}

fn service_binary_arguments() -> Vec<OsString> {
    vec![OsString::from(RUN_AS_SERVICE_FLAG)]
}

/// True when SCM started this process as the service entry point.
pub fn invoked_as_service() -> bool {
    env::args().any(|arg| arg == RUN_AS_SERVICE_FLAG)
}

/// Block in the SCM dispatcher until the service stops.
pub fn run_dispatcher() -> Result<()> {
    windows_service::service_dispatcher::start(SERVICE_NAME, ffi_service_main)
        .context("start Windows service dispatcher")
}

define_windows_service!(ffi_service_main, service_main);

fn service_main(_arguments: Vec<OsString>) {
    if let Err(err) = run_service() {
        log::error!("Service failed: {err:#}");
    }
}

fn run_service() -> Result<()> {
    set_working_directory_to_install_dir()?;
    init_service_file_logger()?;

    let status_handle = service_control_handler::register(
        SERVICE_NAME,
        move |control_event| match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                log::info!("Service stop requested");
                core::request_shutdown();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        },
    )
    .context("register service control handler")?;

    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StartPending,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: windows_service::service::ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::from_secs(10),
            process_id: None,
        })
        .context("set service status StartPending")?;

    let config_path = Config::default_config_path();
    log::info!(
        "Starting packet router (config: {})",
        config_path.display()
    );
    let config = Config::load(&config_path)?;
    let router = PacketRouter::with_interfaces(config, config_path)
        .context("enumerate network interfaces for routing")?;

    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: windows_service::service::ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        })
        .context("set service status Running")?;

    let run_result = router.run();

    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StopPending,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: windows_service::service::ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::from_secs(10),
            process_id: None,
        })
        .ok();

    if let Err(err) = run_result {
        log::error!("Router exited with error: {err:#}");
        status_handle
            .set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: ServiceState::Stopped,
                controls_accepted: ServiceControlAccept::empty(),
                exit_code: windows_service::service::ServiceExitCode::Win32(1),
                checkpoint: 0,
                wait_hint: std::time::Duration::default(),
                process_id: None,
            })
            .ok();
        return Err(err);
    }

    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: windows_service::service::ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        })
        .context("set service status Stopped")?;

    log::info!("Service stopped cleanly");
    Ok(())
}

fn open_service_manager(access: ServiceManagerAccess) -> Result<ServiceManager> {
    ServiceManager::local_computer(None::<&str>, access).context("open local Service Control Manager")
}

pub fn is_installed() -> Result<bool> {
    let manager = open_service_manager(ServiceManagerAccess::CONNECT)?;
    Ok(manager
        .open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)
        .is_ok())
}

pub fn query_state() -> Result<ServiceState> {
    let manager = open_service_manager(ServiceManagerAccess::CONNECT)?;
    let service = manager
        .open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)
        .context("open roust Windows service (is it installed?)")?;
    let status = service
        .query_status()
        .context("query roust service status")?;
    Ok(status.current_state)
}

pub fn install(auto_start: bool) -> Result<()> {
    if is_installed()? {
        return Err(anyhow!(
            "Windows service \"{SERVICE_NAME}\" is already installed. Run `roust --uninstall-service` first."
        ));
    }

    let manager = open_service_manager(ServiceManagerAccess::CREATE_SERVICE)?;
    let exe_path = env::current_exe().context("resolve roust.exe for service registration")?;

    let service_info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: if auto_start {
            ServiceStartType::AutoStart
        } else {
            ServiceStartType::OnDemand
        },
        error_control: ServiceErrorControl::Normal,
        executable_path: exe_path.clone(),
        launch_arguments: service_binary_arguments(),
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };

    manager
        .create_service(&service_info, ServiceAccess::CHANGE_CONFIG)
        .context("create roust Windows service")?;

    println!(
        "Installed Windows service \"{SERVICE_DISPLAY_NAME}\" ({SERVICE_NAME})."
    );
    println!(
        "  Binary: \"{}\" {}",
        exe_path.display(),
        RUN_AS_SERVICE_FLAG
    );
    println!("  Start the service from the Roust app.");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    if !is_installed()? {
        return Err(anyhow!(
            "Windows service \"{SERVICE_NAME}\" is not installed."
        ));
    }

    let _ = stop();

    let manager = open_service_manager(ServiceManagerAccess::CONNECT)?;
    let service = manager
        .open_service(SERVICE_NAME, ServiceAccess::DELETE | ServiceAccess::STOP)
        .context("open roust service for uninstall")?;
    service.delete().context("delete roust Windows service")?;
    println!("Uninstalled Windows service \"{SERVICE_NAME}\".");
    Ok(())
}

pub fn start() -> Result<()> {
    start_service()?;
    println!("Started Windows service \"{SERVICE_NAME}\".");
    Ok(())
}

fn start_service() -> Result<()> {
    if !is_installed()? {
        return Err(anyhow!(
            "Windows service \"{SERVICE_NAME}\" is not installed. Re-run installer.ps1 as Administrator or run `roust --install-service`."
        ));
    }
    let manager = open_service_manager(ServiceManagerAccess::CONNECT)?;
    let service = manager
        .open_service(SERVICE_NAME, ServiceAccess::START)
        .context("open roust service for start")?;
    service
        .start(&service_binary_arguments())
        .context("start roust service")?;
    Ok(())
}

const FORCE_STOP_TIMEOUT: Duration = Duration::from_secs(30);
const FORCE_STOP_POLL: Duration = Duration::from_millis(500);
const RESTART_START_ATTEMPTS: u32 = 3;

fn service_is_active(state: ServiceState) -> bool {
    !matches!(state, ServiceState::Stopped)
}

fn terminate_process(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F", "/T"])
        .output();
}

/// Kill every `roust.exe` except this process (orphaned routers, stuck service).
fn kill_other_roust_processes() {
    let self_pid = std::process::id();
    let script = format!(
        "Get-Process -Name roust -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne {self_pid} }} | Stop-Process -Force -ErrorAction SilentlyContinue"
    );
    let _ = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .status();
}

/// Stop the Windows service and wait; kill the service process if SCM stop hangs.
fn force_stop() -> Result<()> {
    if !is_installed()? {
        kill_other_roust_processes();
        return Ok(());
    }

    let Ok(manager) = open_service_manager(ServiceManagerAccess::CONNECT) else {
        kill_other_roust_processes();
        return Ok(());
    };
    let Ok(service) = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
    ) else {
        kill_other_roust_processes();
        return Ok(());
    };

    if let Ok(status) = service.query_status() {
        if service_is_active(status.current_state) {
            let _ = service.stop();
        }
    }

    let deadline = Instant::now() + FORCE_STOP_TIMEOUT;
    loop {
        let Ok(status) = service.query_status() else {
            break;
        };
        if status.current_state == ServiceState::Stopped {
            break;
        }
        if Instant::now() >= deadline {
            log::warn!(
                "Service \"{SERVICE_NAME}\" did not stop within {}s; terminating process",
                FORCE_STOP_TIMEOUT.as_secs()
            );
            if let Some(pid) = status.process_id {
                terminate_process(pid);
            }
            kill_other_roust_processes();
            thread::sleep(Duration::from_secs(1));
            break;
        }
        thread::sleep(FORCE_STOP_POLL);
    }

    kill_other_roust_processes();
    Ok(())
}

pub fn stop() -> Result<()> {
    let installed = is_installed().unwrap_or(false);
    let active_before = if installed {
        query_state()
            .ok()
            .map(service_is_active)
            .unwrap_or(false)
    } else {
        false
    };

    force_stop()?;

    if !installed {
        println!("Service \"{SERVICE_NAME}\" is not installed.");
    } else if active_before {
        println!("Stopped Windows service \"{SERVICE_NAME}\".");
    } else {
        println!("Service \"{SERVICE_NAME}\" is already stopped.");
    }

    Ok(())
}

pub fn restart() -> Result<()> {
    if !is_installed()? {
        return Err(anyhow!(
            "Windows service \"{SERVICE_NAME}\" is not installed. Re-run installer.ps1 as Administrator or run `roust --install-service`."
        ));
    }

    force_stop().context("force stop roust service before restart")?;

    let mut last_err = None;
    for attempt in 1..=RESTART_START_ATTEMPTS {
        match start_service() {
            Ok(()) => {
                println!("Restarted Windows service \"{SERVICE_NAME}\".");
                return Ok(());
            }
            Err(err) => {
                if attempt < RESTART_START_ATTEMPTS {
                    log::warn!(
                        "Start attempt {attempt}/{RESTART_START_ATTEMPTS} failed ({err:#}); retrying after force stop"
                    );
                    force_stop().ok();
                    thread::sleep(Duration::from_secs(1));
                }
                last_err = Some(err);
            }
        }
    }

    Err(last_err.unwrap()).context("start roust service after force restart")
}

pub fn is_active() -> Result<bool> {
    if !is_installed()? {
        return Ok(false);
    }
    Ok(matches!(query_state()?, ServiceState::Running))
}
