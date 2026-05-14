fn main() {
    #[cfg(target_os = "windows")]
    {
        // Get the SDK path - user should have WinDivert-2.2.2-A in repo root
        let sdk_path = std::path::Path::new("WinDivert-2.2.2-A");

        if !sdk_path.exists() {
            eprintln!(
                "Warning: WinDivert SDK not found at {:?}",
                sdk_path.canonicalize().unwrap_or_default()
            );
            eprintln!("Download WinDivert from: https://www.reqrypt.org/windivert.html");
            eprintln!("Extract to: WinDivert-2.2.2-A/");
        }

        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        {
            println!("cargo:rustc-link-search=native={}/x64", sdk_path.display());
            println!("cargo:rustc-link-lib=WinDivert");
        }

        #[cfg(all(target_os = "windows", target_arch = "x86"))]
        {
            println!("cargo:rustc-link-search=native={}/x86", sdk_path.display());
            println!("cargo:rustc-link-lib=WinDivert");
        }
    }
    
    // Tell cargo to invalidate the built crate if this file changes
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=WinDivert-2.2.2-A/");
}
