use anyhow::{Context, Result};
use clap::Parser;
use roust::setup::{self, SetupOptions};
use std::env;
use std::path::PathBuf;

/// Post-install helper: WinDivert, IP lists, and PATH. Used by the Inno Setup wizard and manual runs.
#[derive(Parser)]
#[command(name = "roust-setup")]
#[command(about = "Configure roust after install (WinDivert, IP lists, PATH)")]
struct Cli {
    /// Install folder (defaults to the directory containing this executable).
    #[arg(long, value_name = "DIR")]
    dir: Option<PathBuf>,

    /// Download and run rustup-init when rustc is missing (off by default for end-user installs).
    #[arg(long)]
    install_rust: bool,

    /// Do not append the install folder to the user PATH.
    #[arg(long)]
    skip_path: bool,

    /// Do not download Iran / private IP list files (offline install).
    #[arg(long)]
    skip_lists: bool,

    /// Do not download WinDivert when DLLs are not already bundled.
    #[arg(long)]
    skip_windivert: bool,

    /// Remove the install folder from the user PATH (run during uninstall).
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

    setup::run(&options)
}
