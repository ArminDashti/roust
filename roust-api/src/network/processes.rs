//! Enumerate running processes for the app-bind picker.

use anyhow::{Context, Result};
use serde::Serialize;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

#[derive(Debug, Clone, Serialize)]
pub struct ProcessItem {
    pub pid: u32,
    pub image_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exe_path: Option<String>,
}

/// List running processes. Inaccessible system processes are skipped (path may be null).
pub fn list_processes() -> Result<Vec<ProcessItem>> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .context("CreateToolhelp32Snapshot")?;
        let _snap_guard = HandleGuard(snapshot);

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut items = Vec::new();
        if Process32FirstW(snapshot, &mut entry).is_err() {
            return Ok(items);
        }

        loop {
            let pid = entry.th32ProcessID;
            if pid != 0 {
                let image_name = wchar_to_string(&entry.szExeFile);
                if !image_name.is_empty() {
                    let exe_path = query_exe_path(pid);
                    items.push(ProcessItem {
                        pid,
                        image_name,
                        exe_path,
                    });
                }
            }

            if Process32NextW(snapshot, &mut entry).is_err() {
                break;
            }
        }

        items.sort_by(|a, b| {
            a.image_name
                .to_ascii_lowercase()
                .cmp(&b.image_name.to_ascii_lowercase())
                .then(a.pid.cmp(&b.pid))
        });
        Ok(items)
    }
}

/// Find a full path for an image name from a currently running process (case-insensitive).
pub fn resolve_image_name_to_path(image_name: &str) -> Option<String> {
    let needle = image_name.trim();
    if needle.is_empty() {
        return None;
    }
    let Ok(list) = list_processes() else {
        return None;
    };
    list.into_iter().find_map(|p| {
        if p.image_name.eq_ignore_ascii_case(needle) {
            p.exe_path
        } else {
            None
        }
    })
}

fn query_exe_path(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let _guard = HandleGuard(handle);
        let mut buf = vec![0u16; MAX_PATH as usize];
        let mut size = buf.len() as u32;
        QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size)
            .ok()?;
        if size == 0 {
            return None;
        }
        Some(OsString::from_wide(&buf[..size as usize]).to_string_lossy().into_owned())
    }
}

fn wchar_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    OsString::from_wide(&buf[..len]).to_string_lossy().into_owned()
}

struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
