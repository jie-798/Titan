use crate::ffi::*;
use libloading::{Library, Symbol};
use std::ffi::{c_void, OsStr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

const WINTUN_MIN_RING_CAPACITY: u32 = 0x20000;
const WINTUN_MAX_RING_CAPACITY: u32 = 0x4000000;
const WINTUN_MAX_IP_PACKET_SIZE: usize = 0xFFFF;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unsupported platform")]
    UnsupportedPlatform,
    #[error("wintun dll not found")]
    DllNotFound,
    #[error("failed to load wintun dll: {0}")]
    LoadDll(String),
    #[error("failed to resolve wintun symbol {0}")]
    ResolveSymbol(String),
    #[error("wintun operation failed: {0}")]
    Operation(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct WintunLibrary {
    inner: Arc<Inner>,
}

struct Inner {
    _library: Library,
    api: Api,
}

#[derive(Clone, Copy)]
struct Api {
    create_adapter: WintunCreateAdapterFn,
    open_adapter: WintunOpenAdapterFn,
    close_adapter: WintunCloseAdapterFn,
    delete_driver: WintunDeleteDriverFn,
    get_adapter_luid: Option<WintunGetAdapterLuidFn>,
    get_running_driver_version: WintunGetRunningDriverVersionFn,
    set_logger: WintunSetLoggerFn,
    start_session: WintunStartSessionFn,
    end_session: WintunEndSessionFn,
    get_read_wait_event: WintunGetReadWaitEventFn,
    receive_packet: WintunReceivePacketFn,
    release_receive_packet: WintunReleaseReceivePacketFn,
    allocate_send_packet: WintunAllocateSendPacketFn,
    send_packet: WintunSendPacketFn,
}

pub struct Adapter {
    library: Arc<Inner>,
    handle: RawAdapterHandle,
}

pub struct Session {
    library: Arc<Inner>,
    handle: RawSessionHandle,
}

pub struct ReceivePacket<'a> {
    session: &'a Session,
    ptr: *mut u8,
    len: u32,
}

pub struct SendPacket<'a> {
    session: &'a Session,
    ptr: *mut u8,
    len: u32,
}

impl WintunLibrary {
    pub fn load_default() -> Result<Self> {
        load_default_dll()
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        load_from_dll(path.as_ref())
    }

    pub fn create_adapter(
        &self,
        name: &str,
        tunnel_type: &str,
        requested_guid: Option<&Guid>,
    ) -> Result<Adapter> {
        let name = widestring(name);
        let tunnel_type = widestring(tunnel_type);
        let guid_ptr = requested_guid.map_or(std::ptr::null(), |guid| guid as *const Guid);
        let handle = unsafe {
            (self.inner.api.create_adapter)(name.as_ptr(), tunnel_type.as_ptr(), guid_ptr)
        };
        if handle.is_null() {
            return Err(last_error("WintunCreateAdapter failed"));
        }
        Ok(Adapter {
            library: Arc::clone(&self.inner),
            handle,
        })
    }

    pub fn open_adapter(&self, name: &str) -> Result<Adapter> {
        let name = widestring(name);
        let handle = unsafe { (self.inner.api.open_adapter)(name.as_ptr()) };
        if handle.is_null() {
            return Err(last_error("WintunOpenAdapter failed"));
        }
        Ok(Adapter {
            library: Arc::clone(&self.inner),
            handle,
        })
    }

    pub fn running_driver_version(&self) -> Option<u32> {
        let version = unsafe { (self.inner.api.get_running_driver_version)() };
        if version == 0 { None } else { Some(version) }
    }

    pub fn delete_driver(&self) -> Result<()> {
        let ok = unsafe { (self.inner.api.delete_driver)() } != 0;
        if ok {
            Ok(())
        } else {
            Err(last_error("WintunDeleteDriver failed"))
        }
    }

    pub fn set_logger(&self, callback: WintunLoggerCallback) {
        unsafe { (self.inner.api.set_logger)(callback) }
    }
}

impl Adapter {
    pub fn start_session(&self, capacity: u32) -> Result<Session> {
        if !capacity.is_power_of_two() || capacity < WINTUN_MIN_RING_CAPACITY || capacity > WINTUN_MAX_RING_CAPACITY {
            return Err(Error::Operation(format!(
                "invalid session capacity {capacity}, expected power of two between {WINTUN_MIN_RING_CAPACITY} and {WINTUN_MAX_RING_CAPACITY}"
            )));
        }
        let handle = unsafe { (self.library.api.start_session)(self.handle, capacity) };
        if handle.is_null() {
            return Err(last_error("WintunStartSession failed"));
        }
        Ok(Session {
            library: Arc::clone(&self.library),
            handle,
        })
    }

    pub fn luid(&self) -> Result<NetLuid> {
        let mut luid = NetLuid::default();
        let get_adapter_luid = self.library.api.get_adapter_luid.ok_or_else(|| {
            Error::Operation("WintunGetAdapterLuid is not available in this DLL".into())
        })?;
        unsafe { get_adapter_luid(self.handle, &mut luid) };
        Ok(luid)
    }
}

impl Session {
    pub fn read_wait_event(&self) -> *mut c_void {
        unsafe { (self.library.api.get_read_wait_event)(self.handle) }
    }

    pub fn receive_packet(&self) -> Result<Option<ReceivePacket<'_>>> {
        let mut len: u32 = 0;
        let ptr = unsafe { (self.library.api.receive_packet)(self.handle, &mut len) };
        if ptr.is_null() {
            let err = std::io::Error::last_os_error();
            if let Some(code) = err.raw_os_error() {
                if code == 259 {
                    return Ok(None);
                }
                if code == 38 {
                    return Err(Error::Operation("wintun adapter is terminating".into()));
                }
            }
            return Err(last_error("WintunReceivePacket failed"));
        }
        Ok(Some(ReceivePacket {
            session: self,
            ptr,
            len,
        }))
    }

    pub fn allocate_send_packet(&self, len: usize) -> Result<SendPacket<'_>> {
        if len > WINTUN_MAX_IP_PACKET_SIZE {
            return Err(Error::Operation(format!(
                "packet too large: {len} bytes, max is {WINTUN_MAX_IP_PACKET_SIZE}"
            )));
        }
        let ptr = unsafe { (self.library.api.allocate_send_packet)(self.handle, len as u32) };
        if ptr.is_null() {
            return Err(last_error("WintunAllocateSendPacket failed"));
        }
        Ok(SendPacket {
            session: self,
            ptr,
            len: len as u32,
        })
    }
}

impl<'a> ReceivePacket<'a> {
    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len()) }
    }
}

impl<'a> SendPacket<'a> {
    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len()) }
    }

    pub fn commit(self) {
        let packet = self;
        unsafe { (packet.session.library.api.send_packet)(packet.session.handle, packet.ptr) };
        std::mem::forget(packet);
    }
}

impl Drop for ReceivePacket<'_> {
    fn drop(&mut self) {
        unsafe { (self.session.library.api.release_receive_packet)(self.session.handle, self.ptr) }
    }
}

impl Drop for Adapter {
    fn drop(&mut self) {
        unsafe { (self.library.api.close_adapter)(self.handle) };
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { (self.library.api.end_session)(self.handle) };
    }
}

#[cfg(windows)]
fn load_default_dll() -> Result<WintunLibrary> {
    let candidates = default_dll_candidates();
    for path in candidates {
        if path.is_file() {
            return load_from_dll(&path);
        }
    }
    Err(Error::DllNotFound)
}

#[cfg(not(windows))]
fn load_default_dll() -> Result<WintunLibrary> {
    Err(Error::UnsupportedPlatform)
}

#[cfg(windows)]
fn load_from_dll(path: &Path) -> Result<WintunLibrary> {
    let library = unsafe { Library::new(path) }
        .map_err(|err| Error::LoadDll(format!("{}: {}", path.display(), err)))?;
    let api = unsafe { load_api(&library) }?;
    Ok(WintunLibrary {
        inner: Arc::new(Inner {
            _library: library,
            api,
        }),
    })
}

#[cfg(not(windows))]
fn load_from_dll(_path: &Path) -> Result<WintunLibrary> {
    Err(Error::UnsupportedPlatform)
}

#[cfg(windows)]
unsafe fn load_api(library: &Library) -> Result<Api> {
    macro_rules! symbol {
        ($name:literal, $ty:ty) => {{
            let sym: Symbol<$ty> = library
                .get(concat!($name, "\0").as_bytes())
                .map_err(|err| Error::ResolveSymbol(format!("{}: {}", $name, err)))?;
            *sym
        }};
    }
    macro_rules! optional_symbol {
        ($name:literal, $ty:ty) => {{
            match library.get::<$ty>(concat!($name, "\0").as_bytes()) {
                Ok(sym) => Some(*sym),
                Err(_) => None,
            }
        }};
    }

    Ok(Api {
        create_adapter: symbol!("WintunCreateAdapter", WintunCreateAdapterFn),
        open_adapter: symbol!("WintunOpenAdapter", WintunOpenAdapterFn),
        close_adapter: symbol!("WintunCloseAdapter", WintunCloseAdapterFn),
        delete_driver: symbol!("WintunDeleteDriver", WintunDeleteDriverFn),
        get_adapter_luid: optional_symbol!("WintunGetAdapterLuid", WintunGetAdapterLuidFn),
        get_running_driver_version: symbol!(
            "WintunGetRunningDriverVersion",
            WintunGetRunningDriverVersionFn
        ),
        set_logger: symbol!("WintunSetLogger", WintunSetLoggerFn),
        start_session: symbol!("WintunStartSession", WintunStartSessionFn),
        end_session: symbol!("WintunEndSession", WintunEndSessionFn),
        get_read_wait_event: symbol!("WintunGetReadWaitEvent", WintunGetReadWaitEventFn),
        receive_packet: symbol!("WintunReceivePacket", WintunReceivePacketFn),
        release_receive_packet: symbol!(
            "WintunReleaseReceivePacket",
            WintunReleaseReceivePacketFn
        ),
        allocate_send_packet: symbol!("WintunAllocateSendPacket", WintunAllocateSendPacketFn),
        send_packet: symbol!("WintunSendPacket", WintunSendPacketFn),
    })
}

#[cfg(not(windows))]
unsafe fn load_api(_library: &Library) -> Result<Api> {
    Err(Error::UnsupportedPlatform)
}

#[cfg(windows)]
fn default_dll_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(repo_wintun_path());
    if let Some(path) = env_wintun_path() {
        candidates.push(path);
    }
    if let Some(path) = exe_wintun_path() {
        candidates.push(path);
    }
    candidates
}

#[cfg(windows)]
fn repo_wintun_path() -> PathBuf {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("third_party")
        .join("wintun")
        .join("bin")
        .join(arch_dir())
        .join("wintun.dll")
}

#[cfg(windows)]
fn env_wintun_path() -> Option<PathBuf> {
    std::env::var_os("TITAN_WINTUN_DLL").map(PathBuf::from)
}

#[cfg(windows)]
fn exe_wintun_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("wintun.dll")))
}

#[cfg(windows)]
fn arch_dir() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else {
        "amd64"
    }
}

#[cfg(windows)]
fn widestring(value: &str) -> Vec<u16> {
    use std::os::windows::prelude::OsStrExt;
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn last_error(context: &str) -> Error {
    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(code) => Error::Operation(format!("{context}: {err} (os error {code})")),
        None => Error::Operation(format!("{context}: {err}")),
    }
}
