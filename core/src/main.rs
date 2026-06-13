use anyhow::Result;
use roust::service;
use std::env;
use std::process;

const INSTALL_SERVICE_FLAG: &str = "--install-service";
const UNINSTALL_SERVICE_FLAG: &str = "--uninstall-service";

fn main() -> Result<()> {
    if service::invoked_as_service() {
        return service::run_dispatcher();
    }

    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == INSTALL_SERVICE_FLAG) {
        return service::install(false);
    }
    if args.iter().any(|arg| arg == UNINSTALL_SERVICE_FLAG) {
        return service::uninstall();
    }

    eprintln!(
        "roust runs as a Windows service. Use the Roust app to manage rules and service state."
    );
    eprintln!("Installer flags: --install-service, --uninstall-service");
    process::exit(1);
}
