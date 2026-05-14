//! WinDivert capture → apply rules (optional IPv4 destination rewrite) → reinject.

use crate::core::PacketRouter;
use anyhow::{anyhow, Result};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(windows)]
use std::sync::atomic::Ordering;

#[cfg(not(windows))]
pub fn run_intercept_loop(_router: &PacketRouter, _run: Arc<AtomicBool>) -> Result<()> {
    Err(anyhow!(
        "Packet interception requires Windows with WinDivert driver and WinDivert.dll installed"
    ))
}

#[cfg(windows)]
pub fn run_intercept_loop(router: &PacketRouter, run: Arc<AtomicBool>) -> Result<()> {
    use std::ffi::CString;
    use windows::Win32::Foundation::INVALID_HANDLE_VALUE;

    type Handle = *mut std::ffi::c_void;

    const WINDIVERT_LAYER_NETWORK: u8 = 0;
    const WINDIVERT_SHUTDOWN_RECV: u32 = 0x0001;

    #[repr(C)]
    struct WinDivertAddress([u8; 256]);

    #[link(name = "WinDivert")]
    extern "system" {
        fn WinDivertOpen(
            filter: *const i8,
            layer: u8,
            priority: i16,
            flags: u64,
        ) -> Handle;
        fn WinDivertRecv(
            handle: Handle,
            ppacket: *mut u8,
            packet_len: u32,
            precv_len: *mut u32,
            paddr: *mut WinDivertAddress,
        ) -> i32;
        fn WinDivertSend(
            handle: Handle,
            ppacket: *const u8,
            packet_len: u32,
            psend_len: *mut u32,
            paddr: *const WinDivertAddress,
        ) -> i32;
        fn WinDivertShutdown(handle: Handle, how: u32) -> i32;
        fn WinDivertClose(handle: Handle) -> i32;
        fn WinDivertHelperCalcChecksums(
            ppacket: *mut u8,
            packet_len: u32,
            paddr: *mut WinDivertAddress,
            flags: u64,
        ) -> i32;
    }

    let filter = CString::new("outbound and ip")
        .map_err(|e| anyhow!("invalid WinDivert filter string: {}", e))?;
    let h = unsafe { WinDivertOpen(filter.as_ptr(), WINDIVERT_LAYER_NETWORK, 0, 0) };
    if h.is_null() || h == INVALID_HANDLE_VALUE {
        return Err(anyhow!(
            "WinDivertOpen failed: {} (install WinDivert.sys and run elevated as Administrator)",
            std::io::Error::last_os_error()
        ));
    }

    let handle_shared = Arc::new(std::sync::Mutex::new(Some(h)));
    let handle_for_ctrl = handle_shared.clone();
    let run_for_ctrl = run.clone();
    ctrlc::set_handler(move || {
        if let Ok(guard) = handle_for_ctrl.lock() {
            if let Some(hh) = guard.as_ref() {
                unsafe {
                    let _ = WinDivertShutdown(*hh, WINDIVERT_SHUTDOWN_RECV);
                }
            }
        }
        run_for_ctrl.store(false, Ordering::SeqCst);
    })
    .map_err(|e| anyhow!("failed to register Ctrl+C handler: {}", e))?;

    let mut packet = vec![0u8; 0xFFFF];

    let loop_result = loop {
        if !run.load(Ordering::SeqCst) {
            break Ok(());
        }
        let h_recv = {
            let guard = handle_shared.lock().map_err(|e| anyhow!("lock: {}", e))?;
            match *guard {
                None => break Ok(()),
                Some(hh) => hh,
            }
        };

        let mut recv_len: u32 = 0;
        let mut addr = WinDivertAddress([0u8; 256]);
        let ok = unsafe {
            WinDivertRecv(
                h_recv,
                packet.as_mut_ptr(),
                packet.len() as u32,
                &mut recv_len,
                &mut addr,
            )
        };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            if !run.load(Ordering::SeqCst) {
                break Ok(());
            }
            break Err(anyhow!("WinDivertRecv failed: {}", err));
        }

        let len = recv_len as usize;
        if len > packet.len() {
            continue;
        }

        let rewrote = router.apply_routing_rule(&mut packet[..len]);
        if rewrote {
            unsafe {
                let _ = WinDivertHelperCalcChecksums(
                    packet.as_mut_ptr(),
                    len as u32,
                    &mut addr,
                    0,
                );
            }
        }

        let mut send_len: u32 = 0;
        let ok_send = unsafe {
            WinDivertSend(
                h_recv,
                packet.as_ptr(),
                len as u32,
                &mut send_len,
                &addr,
            )
        };
        if ok_send == 0 {
            break Err(anyhow!(
                "WinDivertSend failed: {}",
                std::io::Error::last_os_error()
            ));
        }
    };

    if let Ok(mut guard) = handle_shared.lock() {
        if let Some(hh) = guard.take() {
            unsafe {
                let _ = WinDivertShutdown(hh, WINDIVERT_SHUTDOWN_RECV);
                let _ = WinDivertClose(hh);
            }
        }
    }

    loop_result
}
