//! FFI bindings and safe wrapper for the WinDivert 2.x packet capture library.
//!
//! The raw `extern "C"` block declares symbols that link against `WinDivert.lib`
//! (set up by `build.rs`).  The [`safe`] module wraps the raw handle in an
//! RAII type with `Send + Sync` so it can be shared across threads.

use std::ffi::{c_void, CString};
use std::ptr;

pub type HANDLE = *mut c_void;
pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as *mut c_void;

// ── WinDivert layer constant ───────────────────────────────────────────────
pub const WINDIVERT_LAYER_NETWORK: i32 = 0;

// ── WinDivert shutdown constant ────────────────────────────────────────────
pub const WINDIVERT_SHUTDOWN_RECV: i32 = 0x1;

// ── Maximum packet buffer size (40 + 0xFFFF) ──────────────────────────────
pub const WINDIVERT_MTU_MAX: usize = 40 + 0xFFFF;

// ── Network layer data (first 8 bytes of the WINDIVERT_ADDRESS union) ──────
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct WinDivertDataNetwork {
    pub if_idx: u32,
    pub sub_if_idx: u32,
}

/// Mirrors the C `WINDIVERT_ADDRESS` structure (80 bytes total).
///
/// Layout (matches MSVC x64):
///   i64      Timestamp       (offset  0,  8 bytes)
///   u32      bitfield flags  (offset  8,  4 bytes)
///   u32      Reserved2       (offset 12,  4 bytes)
///   [u8;64]  union           (offset 16, 64 bytes)
///
/// The bitfield word packs (LSB first, MSVC order):
///   Layer(8), Event(8), Sniffed(1), Outbound(1), Loopback(1),
///   Impostor(1), IPv6(1), IPChecksum(1), TCPChecksum(1), UDPChecksum(1),
///   Reserved1(8).
#[repr(C)]
pub struct WinDivertAddress {
    pub timestamp: i64,
    flags: u32,
    reserved2: u32,
    union_data: [u8; 64],
}

impl WinDivertAddress {
    pub fn zeroed() -> Self {
        Self {
            timestamp: 0,
            flags: 0,
            reserved2: 0,
            union_data: [0u8; 64],
        }
    }

    // ── Bitfield accessors (MSVC LSB-first layout) ─────────────────────

    pub fn outbound(&self) -> bool {
        (self.flags >> 17) & 1 != 0
    }

    pub fn set_outbound(&mut self, v: bool) {
        if v {
            self.flags |= 1 << 17;
        } else {
            self.flags &= !(1 << 17);
        }
    }

    pub fn ipv6(&self) -> bool {
        (self.flags >> 20) & 1 != 0
    }

    // ── Network union accessors ────────────────────────────────────────

    pub fn network(&self) -> WinDivertDataNetwork {
        let mut net = WinDivertDataNetwork::default();
        unsafe {
            ptr::copy_nonoverlapping(
                self.union_data.as_ptr(),
                &mut net as *mut WinDivertDataNetwork as *mut u8,
                std::mem::size_of::<WinDivertDataNetwork>(),
            );
        }
        net
    }

    pub fn set_network(&mut self, net: WinDivertDataNetwork) {
        unsafe {
            ptr::copy_nonoverlapping(
                &net as *const WinDivertDataNetwork as *const u8,
                self.union_data.as_mut_ptr(),
                std::mem::size_of::<WinDivertDataNetwork>(),
            );
        }
    }
}

// ── Raw FFI imports (linked via WinDivert.lib from build.rs) ───────────────
extern "C" {
    pub fn WinDivertOpen(
        filter: *const i8,
        layer: i32,
        priority: i16,
        flags: u64,
    ) -> HANDLE;

    pub fn WinDivertRecv(
        handle: HANDLE,
        p_packet: *mut c_void,
        packet_len: u32,
        p_recv_len: *mut u32,
        p_addr: *mut WinDivertAddress,
    ) -> i32;

    pub fn WinDivertSend(
        handle: HANDLE,
        p_packet: *const c_void,
        packet_len: u32,
        p_send_len: *mut u32,
        p_addr: *const WinDivertAddress,
    ) -> i32;

    pub fn WinDivertShutdown(handle: HANDLE, how: i32) -> i32;

    pub fn WinDivertClose(handle: HANDLE) -> i32;

    pub fn WinDivertHelperCalcChecksums(
        p_packet: *mut c_void,
        packet_len: u32,
        p_addr: *mut WinDivertAddress,
        flags: u64,
    ) -> i32;
}

// ── Win32 console control handler (declared directly to avoid adding a
//    windows-crate feature just for one symbol from kernel32) ───────────────
extern "system" {
    pub fn SetConsoleCtrlHandler(
        handler_routine: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
}

// ── Safe wrapper ───────────────────────────────────────────────────────────
pub mod safe {
    use super::*;
    use windows::Win32::Foundation::GetLastError;

    /// RAII wrapper around a WinDivert `HANDLE`.
    ///
    /// Closing is performed automatically via [`Drop`].
    pub struct WinDivertHandle {
        handle: HANDLE,
    }

    // WinDivert operations are safe to call from any thread; the caller is
    // responsible for not interleaving partial reads/writes on the same
    // buffer, which we enforce through the borrow-checker (mutable refs).
    unsafe impl Send for WinDivertHandle {}
    unsafe impl Sync for WinDivertHandle {}

    impl WinDivertHandle {
        /// Open a new WinDivert session.
        pub fn open(filter: &str, layer: i32, priority: i16, flags: u64) -> Result<Self, String> {
            let c_filter =
                CString::new(filter).map_err(|e| format!("invalid filter string: {e}"))?;
            let handle =
                unsafe { WinDivertOpen(c_filter.as_ptr(), layer, priority, flags) };
            if handle == INVALID_HANDLE_VALUE {
                let err = unsafe { GetLastError() };
                return Err(format!(
                    "WinDivertOpen failed (GetLastError = {:?})",
                    err
                ));
            }
            Ok(Self { handle })
        }

        /// Return the raw OS handle (needed for the global Ctrl-C handler).
        pub fn raw(&self) -> HANDLE {
            self.handle
        }

        /// Block until a packet is captured.
        pub fn recv(
            &self,
            buf: &mut [u8],
            addr: &mut WinDivertAddress,
        ) -> Result<u32, String> {
            let mut recv_len: u32 = 0;
            let ok = unsafe {
                WinDivertRecv(
                    self.handle,
                    buf.as_mut_ptr() as *mut c_void,
                    buf.len() as u32,
                    &mut recv_len,
                    addr,
                )
            };
            if ok == 0 {
                let err = unsafe { GetLastError() };
                return Err(format!(
                    "WinDivertRecv failed (GetLastError = {:?})",
                    err
                ));
            }
            Ok(recv_len)
        }

        /// Re-inject (or inject) a packet.
        pub fn send(
            &self,
            packet: &[u8],
            addr: &WinDivertAddress,
        ) -> Result<u32, String> {
            let mut send_len: u32 = 0;
            let ok = unsafe {
                WinDivertSend(
                    self.handle,
                    packet.as_ptr() as *const c_void,
                    packet.len() as u32,
                    &mut send_len,
                    addr,
                )
            };
            if ok == 0 {
                let err = unsafe { GetLastError() };
                return Err(format!(
                    "WinDivertSend failed (GetLastError = {:?})",
                    err
                ));
            }
            Ok(send_len)
        }

        /// Shut down part (or all) of the handle, unblocking pending recv/send.
        pub fn shutdown(&self, how: i32) -> Result<(), String> {
            let ok = unsafe { WinDivertShutdown(self.handle, how) };
            if ok == 0 {
                let err = unsafe { GetLastError() };
                return Err(format!(
                    "WinDivertShutdown failed (GetLastError = {:?})",
                    err
                ));
            }
            Ok(())
        }
    }

    impl Drop for WinDivertHandle {
        fn drop(&mut self) {
            unsafe {
                WinDivertClose(self.handle);
            }
        }
    }

    /// Recalculate IP/TCP/UDP checksums in-place using WinDivert helpers.
    pub fn calc_checksums(
        packet: &mut [u8],
        addr: &mut WinDivertAddress,
    ) -> Result<(), String> {
        let ok = unsafe {
            WinDivertHelperCalcChecksums(
                packet.as_mut_ptr() as *mut c_void,
                packet.len() as u32,
                addr,
                0,
            )
        };
        if ok == 0 {
            return Err("WinDivertHelperCalcChecksums failed".to_string());
        }
        Ok(())
    }
}
