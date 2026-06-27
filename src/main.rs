use anyhow::Result;
use roust::service;
use std::env;
use std::io::{self, IsTerminal, Write};
use std::process;

const INSTALL_SERVICE_FLAG: &str = "--install-service";
const UNINSTALL_SERVICE_FLAG: &str = "--uninstall-service";

/// Keep the console open when the user double-clicks `roust.exe` in Explorer.
fn wait_for_interactive_exit() {
    if io::stderr().is_terminal() {
        let _ = writeln!(io::stderr());
        let _ = writeln!(io::stderr(), "Press Enter to exit...");
        let _ = io::stderr().flush();
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
    }
}

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
    wait_for_interactive_exit();
    process::exit(1);
}
