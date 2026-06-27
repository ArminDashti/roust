use anyhow::{Context, Result};
use clap::Parser;
use roust::setup::{self, SetupOptions};
use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

fn wait_for_interactive_exit() {
    if io::stderr().is_terminal() {
        let _ = writeln!(io::stderr());
        let _ = writeln!(io::stderr(), "Press Enter to exit...");
        let _ = io::stderr().flush();
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
    }
}
#[derive(Parser)]
#[command(name = "roust-setup")]
#[command(about = "Configure roust after install (WinDivert, IP lists, PATH)")]
struct Cli {
    #[arg(long, value_name = "DIR")]
    dir: Option<PathBuf>,
    #[arg(long)]
    install_rust: bool,
    #[arg(long)]
    skip_path: bool,
    #[arg(long)]
    skip_lists: bool,
    #[arg(long)]
    skip_windivert: bool,
    #[arg(long)]
    uninstall_path: bool,
}
fn default_install_dir() -> Result<PathBuf> {
    let exe = env::current_exe().context("resolve path of this executable")?;
    exe.parent()
        .map(std::path::Path::to_path_buf)
        .context("executable path has no parent directory")
}
fn main() -> Result<()> {
    let cli = Cli::parse();
    let install_dir = cli
        .dir
        .or_else(|| default_install_dir().ok())
        .context("--dir or a valid executable location is required")?;
    if cli.uninstall_path {
        setup::unregister_install_dir_from_user_path(&install_dir)
            .context("remove install folder from user PATH")?;
        eprintln!("Removed {} from user PATH.", install_dir.display());
        return Ok(());
    }
    let mut options = SetupOptions::from_env_and_dir(install_dir);
    if cli.install_rust {
        options.install_rust = true;
    }
    if cli.skip_path {
        options.update_path = false;
    }
    if cli.skip_lists {
        options.download_lists = false;
    }
    if cli.skip_windivert {
        options.download_windivert = false;
    }
    let result = setup::run(&options);
    if result.is_ok() {
        wait_for_interactive_exit();
    }
    result
}
