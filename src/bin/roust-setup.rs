use anyhow::{Context, Result}; // Import anyhow helpers for attaching context to errors
use roust::update; // Import shared update routines that fetch JSON and write text lists
use std::env; // Import process environment helpers to read the executable path and optional URLs
use std::fs; // Import filesystem helpers to create folders and write extracted files
use std::io::{self, Cursor, Read}; // Import I/O traits and an in-memory cursor for ZIP bytes
use std::path::{Path, PathBuf}; // Import path types for building output locations safely
use zip::ZipArchive; // Import the ZIP reader type so we can unpack the WinDivert archive

const WINDIVERT_ZIP_URL: &str = "https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip"; // Official WinDivert 2.2.2 bundle URL that matches the expected SDK folder name

fn install_dir() -> Result<PathBuf> {
    let exe = env::current_exe().context("resolve path of this executable")?; // Locate this running program so we can anchor paths beside it
    exe.parent() // Take the parent folder of the executable path if it exists
        .map(Path::to_path_buf) // Convert the optional parent reference into an owned PathBuf when present
        .context("executable path has no parent directory") // Fail with a clear message when the path has no parent
} // Close the install_dir function

fn http_get_bytes(url: &str) -> Result<Vec<u8>> {
    let mut reader = ureq::get(url) // Start a blocking GET request for the given URL string
        .call() // Send the request and return a response object or an error
        .with_context(|| format!("HTTP GET {url}"))? // Attach the URL to any transport-layer failure
        .into_reader(); // Convert the successful response into a readable byte stream
    let mut buf = Vec::new(); // Allocate an empty growable buffer to hold the full download
    reader // Use the response reader as the byte source for the next read operation
        .read_to_end(&mut buf) // Read every remaining byte from the reader into the buffer
        .context("read HTTP response body")?; // Explain failures while draining the response body
    Ok(buf) // Return the completed byte vector to the caller as a success value
} // Close the http_get_bytes function

#[allow(deprecated)] // Silence deprecation warnings because zip 0.6 only exposes sanitized_name here
fn unzip_archive(bytes: &[u8], dest_dir: &Path) -> Result<()> {
    let cursor = Cursor::new(bytes); // Wrap the downloaded bytes so the ZIP reader can seek them
    let mut archive = ZipArchive::new(cursor).context("open WinDivert ZIP archive")?; // Open the archive from the in-memory cursor
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).with_context(|| format!("ZIP entry index {i}"))?; // Open each stored entry by stable index
        let rel = file.sanitized_name(); // Convert the stored entry name into a safe relative path
        let out_path = dest_dir.join(&rel); // Join the destination root with the relative entry path
        if file.name().ends_with('/') {
            fs::create_dir_all(&out_path) // Create directory entries when the ZIP marks a folder
                .with_context(|| format!("create directory {}", out_path.display()))?; // Explain failures when creating a directory entry
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent) // Ensure parent folders exist before writing a file entry
                    .with_context(|| format!("create parent for {}", out_path.display()))?; // Explain failures when creating parent folders
            } // Close the optional parent branch
            let mut outfile = fs::File::create(&out_path) // Create the output file on disk for this ZIP entry
                .with_context(|| format!("create {}", out_path.display()))?; // Explain failures when creating the output file
            io::copy(&mut file, &mut outfile).with_context(|| format!("extract {}", out_path.display()))?; // Stream bytes from the ZIP entry into the output file
        } // Close the file versus directory branch
    } // Close the per-entry loop
    Ok(()) // Signal successful extraction without returning extra data
} // Close the unzip_archive function

fn setup_windivert(install_dir: &Path) -> Result<()> {
    let marker = install_dir.join("WinDivert-2.2.2-A").join("x64").join("WinDivert.dll"); // Build the path that proves WinDivert user binaries are already unpacked
    if marker.is_file() {
        eprintln!("WinDivert already present at {}", marker.display()); // Tell the operator we are skipping download because the DLL exists
        return Ok(()); // Exit early without fetching when the bundle is already installed
    } // Close the early-exit branch when WinDivert is already present
    let zip_url = env::var("ROUST_WINDIVERT_ZIP_URL").unwrap_or_else(|_| WINDIVERT_ZIP_URL.to_string()); // Allow operators to override the WinDivert ZIP download URL
    eprintln!("Downloading WinDivert from {zip_url} ..."); // Print progress so users know a large download is starting
    let zip_bytes = http_get_bytes(&zip_url)?; // Download the entire WinDivert archive into memory
    unzip_archive(&zip_bytes, install_dir).context("extract WinDivert archive")?; // Expand the archive beside the executable
    eprintln!("WinDivert extracted under {}", install_dir.display()); // Confirm where the SDK tree was written on disk
    Ok(()) // Signal that WinDivert setup finished successfully
} // Close the setup_windivert function

fn main() -> Result<()> {
    let dir = install_dir()?; // Resolve the folder next to this executable that should receive assets
    eprintln!("Using install directory: {}", dir.display()); // Echo the chosen install directory for easier troubleshooting

    let logs = dir.join("logs"); // Build the path to the logs folder under the install directory
    fs::create_dir_all(&logs).with_context(|| format!("create logs directory {}", logs.display()))?; // Create the logs folder if it is missing
    eprintln!("Logs directory ready: {}", logs.display()); // Confirm the logs folder path to the operator

    setup_windivert(&dir)?; // Download and unpack WinDivert unless it is already present

    update::run(&dir).context("Iran aggregated lists (JSON download and TXT files)")?; // Fetch Iran IP JSON and write the four list text files plus a JSON copy
    update::run_private_ips(&dir).context("private IP lists (JSON download and TXT files)")?; // Fetch private IP JSON and write the two private list text files plus a JSON copy

    eprintln!("Setup finished. WinDivert, list files, and logs folder are in {}", dir.display()); // Summarize success and repeat the install root path
    Ok(()) // Exit the program with a success status code
} // Close the main function
