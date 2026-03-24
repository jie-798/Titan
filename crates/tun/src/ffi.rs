use core::ffi::c_void;

pub type RawAdapterHandle = *mut c_void;
pub type RawSessionHandle = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NetLuid {
    pub value: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(non_camel_case_types)]
pub enum WintunLoggerLevel {
    Info = 0,
    Warn = 1,
    Err = 2,
}

pub type WintunLoggerCallback =
    Option<unsafe extern "system" fn(WintunLoggerLevel, u64, *const u16)>;

pub type WintunCreateAdapterFn =
    unsafe extern "system" fn(*const u16, *const u16, *const Guid) -> RawAdapterHandle;
pub type WintunOpenAdapterFn = unsafe extern "system" fn(*const u16) -> RawAdapterHandle;
pub type WintunCloseAdapterFn = unsafe extern "system" fn(RawAdapterHandle);
pub type WintunDeleteDriverFn = unsafe extern "system" fn() -> i32;
pub type WintunGetAdapterLuidFn = unsafe extern "system" fn(RawAdapterHandle, *mut NetLuid);
pub type WintunGetRunningDriverVersionFn = unsafe extern "system" fn() -> u32;
pub type WintunSetLoggerFn = unsafe extern "system" fn(WintunLoggerCallback);
pub type WintunStartSessionFn = unsafe extern "system" fn(RawAdapterHandle, u32) -> RawSessionHandle;
pub type WintunEndSessionFn = unsafe extern "system" fn(RawSessionHandle);
pub type WintunGetReadWaitEventFn = unsafe extern "system" fn(RawSessionHandle) -> *mut c_void;
pub type WintunReceivePacketFn = unsafe extern "system" fn(RawSessionHandle, *mut u32) -> *mut u8;
pub type WintunReleaseReceivePacketFn = unsafe extern "system" fn(RawSessionHandle, *const u8);
pub type WintunAllocateSendPacketFn = unsafe extern "system" fn(RawSessionHandle, u32) -> *mut u8;
pub type WintunSendPacketFn = unsafe extern "system" fn(RawSessionHandle, *const u8);

