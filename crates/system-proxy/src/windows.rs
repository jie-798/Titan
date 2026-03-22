use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
use winreg::RegKey;

use crate::{ProxyConfig, ProxyState};

const INTERNET_SETTINGS_PATH: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings";

pub fn get_current_state() -> anyhow::Result<ProxyState> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let settings =
        hkcu.open_subkey_with_flags(INTERNET_SETTINGS_PATH, KEY_QUERY_VALUE | KEY_SET_VALUE)?;

    let enabled = settings
        .get_value::<u32, _>("ProxyEnable")
        .unwrap_or_default()
        != 0;
    let server = settings
        .get_value::<String, _>("ProxyServer")
        .unwrap_or_default();
    let bypass = settings
        .get_value::<String, _>("ProxyOverride")
        .unwrap_or_default();

    Ok(ProxyState {
        enabled,
        server,
        bypass,
    })
}

pub fn apply_proxy(config: &ProxyConfig) -> anyhow::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let settings =
        hkcu.open_subkey_with_flags(INTERNET_SETTINGS_PATH, KEY_QUERY_VALUE | KEY_SET_VALUE)?;

    let proxy_server = format!(
        "http={addr};https={addr};socks={addr}",
        addr = config.addr()
    );
    let bypass = config.bypass.join(";");

    settings.set_value("ProxyEnable", &1u32)?;
    settings.set_value("ProxyServer", &proxy_server)?;
    settings.set_value("ProxyOverride", &bypass)?;
    settings.set_value("AutoDetect", &0u32)?;

    refresh_internet_options();
    Ok(())
}

pub fn restore_state(state: &ProxyState) -> anyhow::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let settings =
        hkcu.open_subkey_with_flags(INTERNET_SETTINGS_PATH, KEY_QUERY_VALUE | KEY_SET_VALUE)?;

    settings.set_value("ProxyEnable", &(if state.enabled { 1u32 } else { 0u32 }))?;
    settings.set_value("ProxyServer", &state.server)?;
    settings.set_value("ProxyOverride", &state.bypass)?;

    refresh_internet_options();
    Ok(())
}

pub fn disable_proxy() -> anyhow::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let settings =
        hkcu.open_subkey_with_flags(INTERNET_SETTINGS_PATH, KEY_QUERY_VALUE | KEY_SET_VALUE)?;

    settings.set_value("ProxyEnable", &0u32)?;
    refresh_internet_options();
    Ok(())
}

fn refresh_internet_options() {
    unsafe {
        use windows_sys::Win32::Networking::WinInet::{
            InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
        };

        let _ = InternetSetOptionW(
            std::ptr::null(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null_mut(),
            0,
        );
        let _ = InternetSetOptionW(
            std::ptr::null(),
            INTERNET_OPTION_REFRESH,
            std::ptr::null_mut(),
            0,
        );
    }
}
