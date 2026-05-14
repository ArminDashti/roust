//! Bootstrap the deployment directory next to this executable: WinDivert SDK, IP list files, and logs.
use anyhow::{Context, Result};
use roust::update;
use std::env;
use std::fs;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

/// Official WinDivert binary bundle (matches `build.rs` / local `WinDivert-2.2.2-A` layout).
const WINDIVERT_ZIP_URL: &str =
    "https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip";

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

#[allow(deprecated)] // zip 0.6: `sanitized_name` avoids path traversal from archive entries.
fn unzip_archive(bytes: &[u8], dest_dir: &Path) -> Result<()> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).context("open WinDivert ZIP archive")?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).with_context(|| format!("ZIP entry index {i}"))?;
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
            io::copy(&mut file, &mut outfile).with_context(|| format!("extract {}", out_path.display()))?;
        }
    }
    Ok(())
}

fn setup_windivert(install_dir: &Path) -> Result<()> {
    let marker = install_dir.join("WinDivert-2.2.2-A").join("x64").join("WinDivert.dll");
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

fn main() -> Result<()> {
    let dir = install_dir()?;
    eprintln!("Using install directory: {}", dir.display());

    let logs = dir.join("logs");
    fs::create_dir_all(&logs).with_context(|| format!("create logs directory {}", logs.display()))?;
    eprintln!("Logs directory ready: {}", logs.display());

    setup_windivert(&dir)?;

    update::run(&dir).context("Iran aggregated lists (JSON download and TXT files)")?;
    update::run_private_ips(&dir).context("private IP lists (JSON download and TXT files)")?;

    eprintln!("Setup finished. WinDivert, list files, and logs folder are in {}", dir.display());
    Ok(())
}
