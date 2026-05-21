fn main() {
    // Re-run this build script when it changes, when the SDK tree changes, or when the optional WinDivert path override changes.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=ROUST_WINDIVERT_SDK");
    println!("cargo:rerun-if-changed=WinDivert-2.2.2-A/");

    // Stop non-Windows targets immediately: this crate only ships for Windows (WinDivert, Win32 APIs).
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        panic!(
            "roust is Windows-only. Build with a Windows target, e.g. `cargo build --target x86_64-pc-windows-msvc`.\n\
             Current CARGO_CFG_TARGET_OS={:?}",
            target_os
        );
    }

    // Resolve the WinDivert SDK folder: explicit ROUST_WINDIVERT_SDK wins, otherwise use the tree at the repo root.
    let sdk_path = std::env::var_os("ROUST_WINDIVERT_SDK")
        .map(std::path::PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::PathBuf::from("WinDivert-2.2.2-A"));

    if !sdk_path.exists() {
        eprintln!(
            "Warning: WinDivert SDK not found at {:?}",
            sdk_path
                .canonicalize()
                .unwrap_or_else(|_| sdk_path.clone())
        );
        eprintln!("Download WinDivert from: https://www.reqrypt.org/windivert.html");
        eprintln!("Extract it, then either:");
        eprintln!("  - Rename the folder to WinDivert-2.2.2-A and place it in the repository root, or");
        eprintln!("  - Set ROUST_WINDIVERT_SDK to the full path of that folder before running cargo.");
    }

    // Link prebuilt WinDivert import libraries for the Windows architectures we support.
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    match arch.as_str() {
        "x86_64" => {
            println!(
                "cargo:rustc-link-search=native={}/x64",
                sdk_path.display()
            );
            println!("cargo:rustc-link-lib=WinDivert");
        }
        "x86" => {
            println!(
                "cargo:rustc-link-search=native={}/x86",
                sdk_path.display()
            );
            println!("cargo:rustc-link-lib=WinDivert");
        }
        other => {
            eprintln!(
                "Warning: WinDivert vendor libs are only wired for x86_64/x86; target_arch={other}"
            );
        }
    }
}
