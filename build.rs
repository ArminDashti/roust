fn main() {
    // Stop non-Windows targets immediately: this crate only ships for Windows (WinDivert, Win32 APIs).
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        panic!(
            "roust is Windows-only. Build with a Windows target, e.g. `cargo build --target x86_64-pc-windows-msvc`.\n\
             Current CARGO_CFG_TARGET_OS={:?}",
            target_os
        );
    }

    // Locate the WinDivert SDK tree checked in or extracted next to the repo root.
    let sdk_path = std::path::Path::new("WinDivert-2.2.2-A");

    if !sdk_path.exists() {
        eprintln!(
            "Warning: WinDivert SDK not found at {:?}",
            sdk_path.canonicalize().unwrap_or_default()
        );
        eprintln!("Download WinDivert from: https://www.reqrypt.org/windivert.html");
        eprintln!("Extract to: WinDivert-2.2.2-A/");
    }

    // Link prebuilt WinDivert import libraries for the Windows architectures we support.
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    match arch.as_str() {
        "x86_64" => {
            println!("cargo:rustc-link-search=native={}/x64", sdk_path.display());
            println!("cargo:rustc-link-lib=WinDivert");
        }
        "x86" => {
            println!("cargo:rustc-link-search=native={}/x86", sdk_path.display());
            println!("cargo:rustc-link-lib=WinDivert");
        }
        other => {
            eprintln!(
                "Warning: WinDivert vendor libs are only wired for x86_64/x86; target_arch={other}"
            );
        }
    }

    // Tell cargo to rerun this script when the SDK or this file changes.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=WinDivert-2.2.2-A/");
}
