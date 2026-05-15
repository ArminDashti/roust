use anyhow::{anyhow, Context, Result};
use crate::update;
use std::env;
use std::fs;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

pub const WINDIVERT_ZIP_URL: &str =
    "https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip";

/// Options controlling what the post-install / setup helper does.
#[derive(Debug, Clone)]
pub struct SetupOptions {
    pub install_dir: PathBuf,
    pub install_rust: bool,
    pub update_path: bool,
    pub download_lists: bool,
    pub download_windivert: bool,
}

impl SetupOptions {
    /// Build options from environment variables used by CI and the Inno Setup post-install step.
    pub fn from_env_and_dir(install_dir: PathBuf) -> Self {
        let install_rust = env::var("ROUST_INSTALL_RUST").is_ok()
            && env::var("ROUST_SKIP_RUST").is_err();
        let update_path = env::var("ROUST_SKIP_PATH").is_err();
        let download_lists = env::var("ROUST_SKIP_LISTS").is_err();
        let download_windivert = env::var("ROUST_SKIP_WINDIVERT").is_err();
        Self {
            install_dir,
            install_rust,
            update_path,
            download_lists,
            download_windivert,
        }
    }
}

/// Run the full setup sequence: optional Rust, WinDivert, IP lists, and user PATH.
pub fn run(options: &SetupOptions) -> Result<()> {
    let dir = &options.install_dir;
    eprintln!("Using install directory: {}", dir.display());

    let logs = dir.join("logs");
    fs::create_dir_all(&logs)
        .with_context(|| format!("create logs directory {}", logs.display()))?;
    eprintln!("Logs directory ready: {}", logs.display());

    if options.install_rust {
        ensure_rust_installed_via_rustup()
            .context("ensure Rust toolchain via rustup when rustc is missing")?;
    } else {
        eprintln!("Skipping Rust/rustup (default for release installs; pass --install-rust or set ROUST_INSTALL_RUST to enable).");
    }

    if options.download_windivert {
        setup_windivert(dir)?;
    } else {
        eprintln!("Skipping WinDivert download (ROUST_SKIP_WINDIVERT or --skip-windivert).");
    }

    if options.download_lists {
        update::run(dir).context("Iran aggregated lists (JSON download and TXT files)")?;
        update::run_private_ips(dir).context("private IP lists (JSON download and TXT files)")?;
    } else {
        eprintln!("Skipping IP list downloads (ROUST_SKIP_LISTS or --skip-lists).");
    }

    if options.update_path {
        register_install_dir_on_user_path(dir)
            .context("add install folder to user PATH for PowerShell and cmd")?;
    }

    eprintln!(
        "If PATH was updated, open a new PowerShell window and run `Get-Command roust` to confirm the shell can find roust.exe."
    );
    eprintln!(
        "Setup finished. WinDivert, list files, and logs folder are in {}",
        dir.display()
    );
    Ok(())
}

fn rustup_init_download_url_for_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => {
            "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe"
        }
        "aarch64" => {
            "https://static.rust-lang.org/rustup/dist/aarch64-pc-windows-msvc/rustup-init.exe"
        }
        _ => "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe",
    }
}

fn rustc_responds_on_path() -> bool {
    Command::new("rustc")
        .arg("-V")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn ensure_rust_installed_via_rustup() -> Result<()> {
    if rustc_responds_on_path() {
        eprintln!(
            "Rust is already installed because rustc answered successfully on the current PATH."
        );
        return Ok(());
    }
    eprintln!(
        "Rust was not found on PATH; downloading rustup-init.exe to install the stable toolchain."
    );
    let url = env::var("ROUST_RUSTUP_INIT_URL")
        .unwrap_or_else(|_| rustup_init_download_url_for_arch().to_string());
    let installer_bytes =
        http_get_bytes(&url).context("download rustup-init.exe from static.rust-lang.org")?;
    let installer_path = env::temp_dir().join("roust-rustup-init.exe");
    fs::write(&installer_path, &installer_bytes)
        .with_context(|| format!("write rustup installer to {}", installer_path.display()))?;
    let status = Command::new(&installer_path)
        .arg("-y")
        .arg("--default-toolchain")
        .arg("stable")
        .status()
        .context("failed to execute rustup-init.exe after writing it to the temp folder")?;
    let _ = fs::remove_file(&installer_path);
    if !status.success() {
        anyhow::bail!(
            "rustup-init.exe failed with status {:?}; install Rust manually from https://rustup.rs and rerun setup",
            status.code()
        );
    }
    eprintln!(
        "Rust stable was installed for this user; open a new PowerShell window before expecting rustc on PATH."
    );
    Ok(())
}

fn http_get_bytes(url: &str) -> Result<Vec<u8>> {
    let mut reader = ureq::get(url)
        .call()
        .with_context(|| format!("HTTP GET {url}"))?
        .into_reader();
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .context("read HTTP response body")?;
    Ok(buf)
}

#[allow(deprecated)]
fn unzip_archive(bytes: &[u8], dest_dir: &Path) -> Result<()> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).context("open WinDivert ZIP archive")?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .with_context(|| format!("ZIP entry index {i}"))?;
        let rel = file.sanitized_name();
        let out_path = dest_dir.join(&rel);
        if file.name().ends_with('/') {
            fs::create_dir_all(&out_path)
                .with_context(|| format!("create directory {}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create parent for {}", out_path.display()))?;
            }
            let mut outfile = fs::File::create(&out_path)
                .with_context(|| format!("create {}", out_path.display()))?;
            io::copy(&mut file, &mut outfile)
                .with_context(|| format!("extract {}", out_path.display()))?;
        }
    }
    Ok(())
}

fn setup_windivert(install_dir: &Path) -> Result<()> {
    let marker = install_dir
        .join("WinDivert-2.2.2-A")
        .join("x64")
        .join("WinDivert.dll");
    if marker.is_file() {
        eprintln!("WinDivert already present at {}", marker.display());
        return Ok(());
    }
    let zip_url =
        env::var("ROUST_WINDIVERT_ZIP_URL").unwrap_or_else(|_| WINDIVERT_ZIP_URL.to_string());
    eprintln!("Downloading WinDivert from {zip_url} ...");
    let zip_bytes = http_get_bytes(&zip_url)?;
    unzip_archive(&zip_bytes, install_dir).context("extract WinDivert archive")?;
    eprintln!("WinDivert extracted under {}", install_dir.display());
    Ok(())
}

const PS_APPEND_USER_PATH: &str = r#"$ErrorActionPreference = 'Stop'
$install = [System.IO.Path]::GetFullPath('<<ROUST_INSTALL>>')
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($null -eq $userPath) { $userPath = '' }
$duplicate = $false
foreach ($segment in ($userPath -split ';')) {
  if ([string]::IsNullOrWhiteSpace($segment)) { continue }
  try {
    $full = [System.IO.Path]::GetFullPath($segment)
    if ($full -ieq $install) { $duplicate = $true; break }
  } catch { }
}
if (-not $duplicate) {
  $tail = if ($userPath -eq '' -or $userPath.EndsWith(';')) { '' } else { ';' }
  [Environment]::SetEnvironmentVariable('Path', ($userPath + $tail + $install), 'User')
}
"#;

const PS_REMOVE_USER_PATH: &str = r#"$ErrorActionPreference = 'Stop'
$install = [System.IO.Path]::GetFullPath('<<ROUST_INSTALL>>')
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($null -eq $userPath) { exit 0 }
$segments = @()
foreach ($segment in ($userPath -split ';')) {
  if ([string]::IsNullOrWhiteSpace($segment)) { continue }
  try {
    $full = [System.IO.Path]::GetFullPath($segment)
    if ($full -ieq $install) { continue }
  } catch { }
  $segments += $segment
}
$newPath = ($segments -join ';')
[Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
"#;

fn run_powershell_script(script: &str) -> Result<()> {
    let status = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script)
        .status()
        .context("failed to start powershell.exe for PATH script")?;
    if !status.success() {
        anyhow::bail!(
            "PowerShell PATH script failed with status {:?}",
            status.code()
        );
    }
    Ok(())
}

fn embed_install_dir_in_ps(template: &str, install_dir: &Path) -> Result<String> {
    let path_utf8 = install_dir.to_str().ok_or_else(|| {
        anyhow!("install folder path must be UTF-8 so PowerShell can embed it in the PATH script")
    })?;
    let escaped = path_utf8.replace('\'', "''");
    Ok(template.replacen("<<ROUST_INSTALL>>", &escaped, 1))
}

pub fn register_install_dir_on_user_path(install_dir: &Path) -> Result<()> {
    let script = embed_install_dir_in_ps(PS_APPEND_USER_PATH, install_dir)?;
    run_powershell_script(&script)
}

/// Remove the install folder from the per-user PATH (used during uninstall).
pub fn unregister_install_dir_from_user_path(install_dir: &Path) -> Result<()> {
    let script = embed_install_dir_in_ps(PS_REMOVE_USER_PATH, install_dir)?;
    run_powershell_script(&script)
}
