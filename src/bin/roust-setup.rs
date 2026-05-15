use anyhow::{anyhow, Context, Result};
use roust::update;
use std::env;
use std::fs;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

const WINDIVERT_ZIP_URL: &str =
    "https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip";

// Pick the official rustup bootstrap EXE URL for the host CPU architecture string Cargo exposes.
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

// Return true when `rustc -V` succeeds so we can skip downloading rustup-init.
fn rustc_responds_on_path() -> bool {
    Command::new("rustc")
        .arg("-V")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

// Download and run rustup-init when rustc is missing, unless the operator opted out via env.
fn ensure_rust_installed_via_rustup() -> Result<()> {
    if env::var("ROUST_SKIP_RUST").is_ok() {
        eprintln!(
            "ROUST_SKIP_RUST is set so this installer will not download or run rustup-init.exe."
        );
        return Ok(());
    }
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
    eprintln!("Rust stable was installed for this user; open a new PowerShell window before expecting rustc on PATH.");
    Ok(())
}

fn install_dir() -> Result<PathBuf> {
    let exe = env::current_exe().context("resolve path of this executable")?;
    exe.parent()
        .map(Path::to_path_buf)
        .context("executable path has no parent directory")
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

// Append the install folder to the per-user PATH via PowerShell when it is not already present.
fn register_install_dir_on_user_path(install_dir: &Path) -> Result<()> {
    if env::var("ROUST_SKIP_PATH").is_ok() {
        eprintln!(
            "ROUST_SKIP_PATH is set so the installer will not change your user PATH variable."
        );
        return Ok(());
    }
    let path_utf8 = install_dir.to_str().ok_or_else(|| {
        anyhow!("install folder path must be UTF-8 so PowerShell can embed it in the PATH update script")
    })?;
    let escaped = path_utf8.replace('\'', "''");
    let script = PS_APPEND_USER_PATH.replacen("<<ROUST_INSTALL>>", &escaped, 1);
    let status = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(&script)
        .status()
        .context("failed to start powershell.exe while trying to update the user PATH variable")?;
    if !status.success() {
        anyhow::bail!(
            "PATH update via PowerShell failed with status {:?}; you can still run roust.exe using its full path",
            status.code()
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    let dir = install_dir()?;
    eprintln!("Using install directory: {}", dir.display());

    let logs = dir.join("logs");
    fs::create_dir_all(&logs)
        .with_context(|| format!("create logs directory {}", logs.display()))?;
    eprintln!("Logs directory ready: {}", logs.display());

    ensure_rust_installed_via_rustup()
        .context("ensure Rust toolchain via rustup when rustc is missing")?;

    setup_windivert(&dir)?;

    update::run(&dir).context("Iran aggregated lists (JSON download and TXT files)")?;
    update::run_private_ips(&dir).context("private IP lists (JSON download and TXT files)")?;

    register_install_dir_on_user_path(&dir)
        .context("add install folder to user PATH for PowerShell and cmd")?;

    eprintln!("If PATH was updated, open a new PowerShell window and run `Get-Command roust` to confirm the shell can find roust.exe.");
    eprintln!(
        "Setup finished. WinDivert, list files, and logs folder are in {}",
        dir.display()
    );
    Ok(())
}
