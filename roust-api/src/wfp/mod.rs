//! User-mode WFP ALE filters for per-app NIC binding (fail-closed, IPv4).

use crate::config::{AppBind, AppBindStatus, AppBindStore};
use crate::network::{
    enumerate_interfaces, find_interface, resolve_image_name_to_path, NetworkInterface,
};
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::core::{GUID, PCWSTR, PWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::{
    FwpmEngineClose0, FwpmEngineOpen0, FwpmFilterAdd0, FwpmFilterCreateEnumHandle0,
    FwpmFilterDeleteByKey0, FwpmFilterDestroyEnumHandle0, FwpmFilterEnum0, FwpmFreeMemory0,
    FwpmGetAppIdFromFileName0, FwpmProviderAdd0, FwpmSubLayerAdd0, FwpmTransactionAbort0,
    FwpmTransactionBegin0, FwpmTransactionCommit0, FWPM_ACTION0, FWPM_CONDITION_ALE_APP_ID,
    FWPM_CONDITION_INTERFACE_INDEX, FWPM_DISPLAY_DATA0, FWPM_FILTER0, FWPM_FILTER_CONDITION0,
    FWPM_FILTER_ENUM_TEMPLATE0, FWPM_LAYER_ALE_AUTH_CONNECT_V4,
    FWPM_LAYER_ALE_RESOURCE_ASSIGNMENT_V4, FWPM_PROVIDER0, FWPM_SESSION0, FWPM_SESSION_FLAG_DYNAMIC,
    FWPM_SUBLAYER0, FWP_ACTION_BLOCK, FWP_ACTION_PERMIT, FWP_BYTE_BLOB, FWP_BYTE_BLOB_TYPE,
    FWP_CONDITION_VALUE0, FWP_MATCH_EQUAL, FWP_UINT32, FWP_UINT8, FWP_VALUE0,
};

/// Stable provider key for Roust app-bind filters.
pub const ROUST_PROVIDER_KEY: GUID = GUID::from_u128(0x524f_5553_7420_4170_7042_696e_6473_3101);
/// Stable sublayer key under the Roust provider.
pub const ROUST_SUBLAYER_KEY: GUID = GUID::from_u128(0x524f_5553_7420_4170_7042_696e_6473_3201);

const PROVIDER_NAME: &str = "Roust";
const PROVIDER_DESC: &str = "Roust per-app NIC binding";
const SUBLAYER_NAME: &str = "Roust App Binds";
const SUBLAYER_DESC: &str = "Fail-closed ALE filters for app→NIC policy";
/// RPC_C_AUTHN_WINNT
const AUTH_WINNT: u32 = 10;
/// FWP_E_ALREADY_EXISTS
const FWP_E_ALREADY_EXISTS: u32 = 0x8032_0009;

/// Owns a dynamic WFP session; filters are removed when dropped.
pub struct WfpEngine {
    handle: HANDLE,
    stop: Arc<AtomicBool>,
    watcher: Option<JoinHandle<()>>,
}

impl WfpEngine {
    /// Open WFP, ensure provider/sublayer, sync binds, and watch adapters.
    pub fn start(app_binds_path: &Path) -> Result<Self> {
        let handle = open_engine().context("open WFP engine")?;
        let mut engine = Self {
            handle,
            stop: Arc::new(AtomicBool::new(false)),
            watcher: None,
        };
        engine
            .ensure_provider_sublayer()
            .context("register WFP provider/sublayer")?;
        engine
            .sync_from_path(app_binds_path)
            .context("initial app-bind WFP sync")?;

        let stop = Arc::clone(&engine.stop);
        let path = app_binds_path.to_path_buf();
        // HANDLE is not Send; pass the raw pointer value as isize.
        let handle_bits = engine.handle.0 as isize;
        engine.watcher = Some(thread::spawn(move || {
            let handle = HANDLE(handle_bits as *mut _);
            adapter_watch_loop(handle, path, stop);
        }));
        Ok(engine)
    }

    fn ensure_provider_sublayer(&self) -> Result<()> {
        unsafe {
            let mut name: Vec<u16> = PROVIDER_NAME.encode_utf16().chain(std::iter::once(0)).collect();
            let mut desc: Vec<u16> = PROVIDER_DESC.encode_utf16().chain(std::iter::once(0)).collect();
            let provider = FWPM_PROVIDER0 {
                providerKey: ROUST_PROVIDER_KEY,
                displayData: FWPM_DISPLAY_DATA0 {
                    name: PWSTR(name.as_mut_ptr()),
                    description: PWSTR(desc.as_mut_ptr()),
                },
                ..Default::default()
            };
            let status = FwpmProviderAdd0(self.handle, &provider, None);
            check_add_status(status, "FwpmProviderAdd0")?;

            let mut provider_key = ROUST_PROVIDER_KEY;
            let mut sname: Vec<u16> = SUBLAYER_NAME.encode_utf16().chain(std::iter::once(0)).collect();
            let mut sdesc: Vec<u16> = SUBLAYER_DESC.encode_utf16().chain(std::iter::once(0)).collect();
            let sublayer = FWPM_SUBLAYER0 {
                subLayerKey: ROUST_SUBLAYER_KEY,
                displayData: FWPM_DISPLAY_DATA0 {
                    name: PWSTR(sname.as_mut_ptr()),
                    description: PWSTR(sdesc.as_mut_ptr()),
                },
                providerKey: &mut provider_key,
                weight: 0x100,
                ..Default::default()
            };
            let status = FwpmSubLayerAdd0(self.handle, &sublayer, None);
            check_add_status(status, "FwpmSubLayerAdd0")?;
        }
        Ok(())
    }

    pub fn sync_from_path(&self, path: &Path) -> Result<()> {
        let store = AppBindStore::load(path).with_context(|| format!("load {}", path.display()))?;
        let interfaces = enumerate_interfaces().context("enumerate interfaces for WFP sync")?;
        sync_filters(self.handle, store.get_binds(), &interfaces)
    }
}

impl Drop for WfpEngine {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.watcher.take() {
            let _ = handle.join();
        }
        unsafe {
            let _ = FwpmEngineClose0(self.handle);
        }
    }
}

fn check_win32(status: u32, what: &str) -> Result<()> {
    if status == ERROR_SUCCESS.0 {
        Ok(())
    } else {
        Err(anyhow!("{what} failed: 0x{status:08X}"))
    }
}

fn check_add_status(status: u32, what: &str) -> Result<()> {
    if status == ERROR_SUCCESS.0 || status == FWP_E_ALREADY_EXISTS {
        Ok(())
    } else {
        Err(anyhow!("{what} failed: 0x{status:08X}"))
    }
}

fn open_engine() -> Result<HANDLE> {
    unsafe {
        let session = FWPM_SESSION0 {
            flags: FWPM_SESSION_FLAG_DYNAMIC,
            ..Default::default()
        };
        let mut handle = HANDLE::default();
        let status = FwpmEngineOpen0(PCWSTR::null(), AUTH_WINNT, None, Some(&session), &mut handle);
        check_win32(status, "FwpmEngineOpen0")?;
        Ok(handle)
    }
}

fn adapter_watch_loop(engine: HANDLE, path: PathBuf, stop: Arc<AtomicBool>) {
    let mut last_sig = adapter_signature().unwrap_or_default();
    while !stop.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_secs(5));
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let sig = adapter_signature().unwrap_or_default();
        if sig != last_sig {
            last_sig = sig;
            log::info!("Adapter change detected; resyncing app-bind WFP filters");
            if let Err(err) = (|| -> Result<()> {
                let store = AppBindStore::load(&path)?;
                let interfaces = enumerate_interfaces()?;
                sync_filters(engine, store.get_binds(), &interfaces)
            })() {
                log::error!("App-bind WFP resync failed: {err:#}");
            }
        }
    }
}

fn adapter_signature() -> Result<String> {
    let interfaces = enumerate_interfaces()?;
    let mut parts: Vec<String> = interfaces
        .iter()
        .map(|i| {
            format!(
                "{}:{}:{}",
                i.if_index,
                i.status,
                i.ipv4_address.as_deref().unwrap_or("")
            )
        })
        .collect();
    parts.sort();
    Ok(parts.join("|"))
}

fn sync_filters(engine: HANDLE, binds: &[AppBind], interfaces: &[NetworkInterface]) -> Result<()> {
    unsafe {
        check_win32(FwpmTransactionBegin0(engine, 0), "FwpmTransactionBegin0")?;
        let result = (|| -> Result<()> {
            delete_roust_filters(engine)?;
            for bind in binds {
                add_filters_for_bind(engine, bind, interfaces)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => check_win32(FwpmTransactionCommit0(engine), "FwpmTransactionCommit0"),
            Err(err) => {
                let _ = FwpmTransactionAbort0(engine);
                Err(err)
            }
        }
    }
}

unsafe fn delete_roust_filters(engine: HANDLE) -> Result<()> {
    let mut provider_key = ROUST_PROVIDER_KEY;
    for layer in [
        FWPM_LAYER_ALE_AUTH_CONNECT_V4,
        FWPM_LAYER_ALE_RESOURCE_ASSIGNMENT_V4,
    ] {
        let template = FWPM_FILTER_ENUM_TEMPLATE0 {
            providerKey: &mut provider_key,
            layerKey: layer,
            ..Default::default()
        };
        let mut enum_handle = HANDLE::default();
        let status = FwpmFilterCreateEnumHandle0(engine, Some(&template), &mut enum_handle);
        if status != ERROR_SUCCESS.0 {
            continue;
        }
        loop {
            let mut entries: *mut *mut FWPM_FILTER0 = std::ptr::null_mut();
            let mut num = 0u32;
            let enum_hr = FwpmFilterEnum0(engine, enum_handle, 64, &mut entries, &mut num);
            if enum_hr != ERROR_SUCCESS.0 || num == 0 {
                break;
            }
            for i in 0..num as usize {
                let filter = &**entries.add(i);
                let _ = FwpmFilterDeleteByKey0(engine, &filter.filterKey);
            }
            let mut free_ptr = entries as *mut std::ffi::c_void;
            FwpmFreeMemory0(&mut free_ptr);
        }
        let _ = FwpmFilterDestroyEnumHandle0(engine, enum_handle);
    }
    Ok(())
}

unsafe fn add_filters_for_bind(
    engine: HANDLE,
    bind: &AppBind,
    interfaces: &[NetworkInterface],
) -> Result<()> {
    let Some(app_path) = resolve_bind_exe_path(bind) else {
        log::warn!("Skipping WFP filters for unresolved bind {}", bind.label());
        return Ok(());
    };

    let app_id = get_app_id(&app_path)?;
    let iface = find_interface(interfaces, bind.nic.trim());
    let nic_ok = iface.is_some_and(|i| {
        i.status.eq_ignore_ascii_case("Up")
            && i.ipv4_address
                .as_ref()
                .is_some_and(|ip| !ip.is_empty() && ip != "0.0.0.0")
    });

    for layer in [
        FWPM_LAYER_ALE_AUTH_CONNECT_V4,
        FWPM_LAYER_ALE_RESOURCE_ASSIGNMENT_V4,
    ] {
        if nic_ok {
            let if_index = iface.unwrap().if_index;
            add_filter(
                engine,
                layer,
                &app_id,
                Some(if_index),
                true,
                15,
                &format!("Roust allow {} on if {}", bind.label(), if_index),
            )?;
            add_filter(
                engine,
                layer,
                &app_id,
                None,
                false,
                10,
                &format!("Roust block {} off-NIC", bind.label()),
            )?;
        } else {
            add_filter(
                engine,
                layer,
                &app_id,
                None,
                false,
                15,
                &format!("Roust block {} (NIC down)", bind.label()),
            )?;
        }
    }

    free_app_id(app_id);
    Ok(())
}

fn resolve_bind_exe_path(bind: &AppBind) -> Option<String> {
    if let Some(path) = bind
        .exe_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if Path::new(path).is_file() {
            return Some(path.to_string());
        }
        log::warn!("exe-path not found on disk: {path}");
    }
    let image = bind
        .image_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    // Image-name fallback: resolve via a running process with that image name.
    resolve_image_name_to_path(image)
}

struct AppIdBlob(*mut FWP_BYTE_BLOB);

unsafe fn get_app_id(path: &str) -> Result<AppIdBlob> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut blob: *mut FWP_BYTE_BLOB = std::ptr::null_mut();
    let status = FwpmGetAppIdFromFileName0(PCWSTR(wide.as_ptr()), &mut blob);
    check_win32(status, &format!("FwpmGetAppIdFromFileName0({path})"))?;
    if blob.is_null() {
        return Err(anyhow!("FwpmGetAppIdFromFileName0 returned null for {path}"));
    }
    Ok(AppIdBlob(blob))
}

unsafe fn free_app_id(blob: AppIdBlob) {
    if !blob.0.is_null() {
        let mut free_ptr = blob.0 as *mut std::ffi::c_void;
        FwpmFreeMemory0(&mut free_ptr);
    }
}

unsafe fn add_filter(
    engine: HANDLE,
    layer: GUID,
    app_id: &AppIdBlob,
    if_index: Option<u32>,
    permit: bool,
    weight: u8,
    name: &str,
) -> Result<()> {
    let mut conditions: Vec<FWPM_FILTER_CONDITION0> = Vec::with_capacity(2);

    let mut app_cond = FWPM_FILTER_CONDITION0 {
        fieldKey: FWPM_CONDITION_ALE_APP_ID,
        matchType: FWP_MATCH_EQUAL,
        conditionValue: FWP_CONDITION_VALUE0 {
            r#type: FWP_BYTE_BLOB_TYPE,
            Anonymous: std::mem::zeroed(),
        },
        ..Default::default()
    };
    app_cond.conditionValue.Anonymous.byteBlob = app_id.0;
    conditions.push(app_cond);

    if let Some(idx) = if_index {
        let mut if_cond = FWPM_FILTER_CONDITION0 {
            fieldKey: FWPM_CONDITION_INTERFACE_INDEX,
            matchType: FWP_MATCH_EQUAL,
            conditionValue: FWP_CONDITION_VALUE0 {
                r#type: FWP_UINT32,
                Anonymous: std::mem::zeroed(),
            },
            ..Default::default()
        };
        if_cond.conditionValue.Anonymous.uint32 = idx;
        conditions.push(if_cond);
    }

    let mut name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut provider_key = ROUST_PROVIDER_KEY;
    let mut weight_value = FWP_VALUE0 {
        r#type: FWP_UINT8,
        Anonymous: std::mem::zeroed(),
    };
    weight_value.Anonymous.uint8 = weight;

    let filter = FWPM_FILTER0 {
        displayData: FWPM_DISPLAY_DATA0 {
            name: PWSTR(name_wide.as_mut_ptr()),
            description: PWSTR::null(),
        },
        providerKey: &mut provider_key,
        layerKey: layer,
        subLayerKey: ROUST_SUBLAYER_KEY,
        weight: weight_value,
        numFilterConditions: conditions.len() as u32,
        filterCondition: conditions.as_mut_ptr(),
        action: FWPM_ACTION0 {
            r#type: if permit {
                FWP_ACTION_PERMIT
            } else {
                FWP_ACTION_BLOCK
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let mut id = 0u64;
    let status = FwpmFilterAdd0(engine, &filter, None, Some(&mut id));
    check_win32(status, &format!("FwpmFilterAdd0({name})"))?;
    Ok(())
}

/// Resolve bind status for API listing (does not require WFP elevation).
pub fn resolve_bind_status(bind: &AppBind, interfaces: &[NetworkInterface]) -> AppBindStatus {
    if resolve_bind_exe_path(bind).is_none() {
        return AppBindStatus::Unresolved;
    }
    match find_interface(interfaces, bind.nic.trim()) {
        Some(iface)
            if iface.status.eq_ignore_ascii_case("Up")
                && iface
                    .ipv4_address
                    .as_ref()
                    .is_some_and(|ip| !ip.is_empty() && ip != "0.0.0.0") =>
        {
            AppBindStatus::Healthy
        }
        Some(_) => AppBindStatus::NicDown,
        None => AppBindStatus::Unresolved,
    }
}
