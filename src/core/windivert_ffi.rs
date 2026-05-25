use std::ffi::{c_void, CString};
use std::ptr;
pub type HANDLE = *mut c_void;
pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as *mut c_void;
pub const WINDIVERT_LAYER_NETWORK: i32 = 0;
pub const WINDIVERT_SHUTDOWN_RECV: i32 = 0x1;
pub const WINDIVERT_MTU_MAX: usize = 40 + 0xFFFF;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct WinDivertDataNetwork {
    pub if_idx: u32,
    pub sub_if_idx: u32,
}

#[repr(C)]
pub struct WinDivertAddress {
    pub timestamp: i64,
    flags: u32,
    reserved2: u32,
    union_data: [u8; 64],
}
/// Bit index of `Outbound` in the `WINDIVERT_ADDRESS` flag word (after Layer + Event + Sniffed).
const OUTBOUND_FLAG_BIT: u32 = 17;
const IP_CHECKSUM_VALID_BIT: u32 = 21;
const TCP_CHECKSUM_VALID_BIT: u32 = 22;
const UDP_CHECKSUM_VALID_BIT: u32 = 23;

impl WinDivertAddress {
    pub fn zeroed() -> Self {
        Self {
            timestamp: 0,
            flags: 0,
            reserved2: 0,
            union_data: [0u8; 64],
        }
    }

    /// True when the packet is on the outbound path (kernel → wire).
    pub fn is_outbound(&self) -> bool {
        (self.flags >> OUTBOUND_FLAG_BIT) & 1 != 0
    }

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

    /// Clear valid-checksum flags so `WinDivertHelperCalcChecksums` recomputes after header edits.
    pub fn invalidate_checksum_flags(&mut self) {
        let mask = (1 << IP_CHECKSUM_VALID_BIT)
            | (1 << TCP_CHECKSUM_VALID_BIT)
            | (1 << UDP_CHECKSUM_VALID_BIT);
        self.flags &= !mask;
    }
}

extern "C" {
    pub fn WinDivertOpen(filter: *const i8, layer: i32, priority: i16, flags: u64) -> HANDLE;

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

extern "system" {
    pub fn SetConsoleCtrlHandler(
        handler_routine: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
}

pub mod safe {
    use super::*;
    use windows::Win32::Foundation::GetLastError;
    pub struct WinDivertHandle {
        handle: HANDLE,
    }
    unsafe impl Send for WinDivertHandle {}
    unsafe impl Sync for WinDivertHandle {}

    impl WinDivertHandle {
        pub fn open(filter: &str, layer: i32, priority: i16, flags: u64) -> Result<Self, String> {
            let c_filter =
                CString::new(filter).map_err(|e| format!("invalid filter string: {e}"))?;
            let handle = unsafe { WinDivertOpen(c_filter.as_ptr(), layer, priority, flags) };
            if handle == INVALID_HANDLE_VALUE {
                let err = unsafe { GetLastError() };
                return Err(format!("WinDivertOpen failed (GetLastError = {:?})", err));
            }
            Ok(Self { handle })
        }

        pub fn raw(&self) -> HANDLE {
            self.handle
        }

        pub fn recv(&self, buf: &mut [u8], addr: &mut WinDivertAddress) -> Result<u32, String> {
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
                return Err(format!("WinDivertRecv failed (GetLastError = {:?})", err));
            }
            Ok(recv_len)
        }

        pub fn send(&self, packet: &[u8], addr: &WinDivertAddress) -> Result<u32, String> {
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
                return Err(format!("WinDivertSend failed (GetLastError = {:?})", err));
            }
            Ok(send_len)
        }
    }

    impl Drop for WinDivertHandle {
        fn drop(&mut self) {
            unsafe {
                WinDivertClose(self.handle);
            }
        }
    }

    pub fn calc_checksums(packet: &mut [u8], addr: &mut WinDivertAddress) -> Result<(), String> {
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
