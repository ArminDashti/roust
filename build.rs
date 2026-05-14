fn main() {
    // Re-run this build script when it changes or when the operator changes the optional WinDivert path override.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=ROUST_WINDIVERT_SDK");

    #[cfg(target_os = "windows")]
    {
        // Resolve the WinDivert SDK folder: explicit ROUST_WINDIVERT_SDK wins, otherwise use the tree shipped at the repo root.
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

        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        {
            println!(
                "cargo:rustc-link-search=native={}/x64",
                sdk_path.display()
            );
            println!("cargo:rustc-link-lib=WinDivert");
        }

        #[cfg(all(target_os = "windows", target_arch = "x86"))]
        {
            println!(
                "cargo:rustc-link-search=native={}/x86",
                sdk_path.display()
            );
            println!("cargo:rustc-link-lib=WinDivert");
        }
    }

    // Invalidate the crate build when the default vendored WinDivert tree changes (no-op when using only ROUST_WINDIVERT_SDK).
    println!("cargo:rerun-if-changed=WinDivert-2.2.2-A/");
}
