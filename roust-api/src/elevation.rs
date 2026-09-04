//! Require an elevated (Administrator) process on Windows.

use anyhow::{anyhow, Context, Result};
use std::env;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::process;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HINSTANCE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_NORMAL;

/// True when this process token has the elevated Administrator bit set.
pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        );
        let _ = CloseHandle(token);
        ok.is_ok() && elevation.TokenIsElevated != 0
    }
}

fn to_wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

/// If not elevated, relaunch this executable with a UAC prompt and exit the current process.
pub fn ensure_elevated() -> Result<()> {
    if is_elevated() {
        return Ok(());
    }

    let exe = env::current_exe().context("resolve current executable for elevation")?;
    let args: Vec<String> = env::args().skip(1).collect();
    let params = args.join(" ");

    let file = to_wide(exe.as_os_str());
    let operation = to_wide(OsStr::new("runas"));
    let parameters = to_wide(OsStr::new(&params));
    let working_dir = exe
        .parent()
        .map(|p| to_wide(p.as_os_str()))
        .unwrap_or_else(|| to_wide(OsStr::new("")));

    let status = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(operation.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            PCWSTR(working_dir.as_ptr()),
            SW_NORMAL,
        )
    };

    // ShellExecuteW returns > 32 on success (HINSTANCE cast from integer).
    let code = status.0 as isize;
    if code <= 32 {
        return Err(anyhow!(
            "failed to relaunch as Administrator (ShellExecuteW status {code}). Right-click the app and choose Run as administrator."
        ));
    }

    process::exit(0);
}
