//! Windows login autostart via the per-user Run registry value.
//!
//! Tests and isolated acceptance instances can set `PETSONA_AUTOSTART_VALUE`
//! to a dedicated value name so they never touch the real `Petsona` entry.

#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::ptr::{null, null_mut};

#[cfg(windows)]
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
#[cfg(windows)]
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
};

#[cfg(windows)]
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

#[cfg(windows)]
fn value_name() -> String {
    std::env::var("PETSONA_AUTOSTART_VALUE").unwrap_or_else(|_| "Petsona".to_string())
}

#[cfg(windows)]
fn open_run_key() -> Result<HKEY, String> {
    let subkey = wide(RUN_KEY);
    let mut key: HKEY = null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            &mut key,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(format!("无法打开 HKCU Run（错误 {status}）"));
    }
    Ok(key)
}

#[cfg(windows)]
fn create_run_key() -> Result<HKEY, String> {
    let subkey = wide(RUN_KEY);
    let mut key: HKEY = null_mut();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            null(),
            REG_OPTION_NON_VOLATILE,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            null(),
            &mut key,
            null_mut(),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(format!("无法创建 HKCU Run（错误 {status}）"));
    }
    Ok(key)
}

#[cfg(windows)]
fn current_command() -> Result<Vec<u16>, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("无法读取应用路径：{error}"))?;
    Ok(wide(&format!("\"{}\"", executable.display())))
}

#[cfg(windows)]
pub fn is_enabled() -> bool {
    let Ok(key) = open_run_key() else {
        return false;
    };
    let name = wide(&value_name());
    let mut value_type = 0u32;
    let mut size = 0u32;
    let status = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            null(),
            &mut value_type,
            null_mut(),
            &mut size,
        )
    };
    unsafe { RegCloseKey(key) };
    status == ERROR_SUCCESS || status == 234 // ERROR_MORE_DATA
}

#[cfg(windows)]
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let key = create_run_key()?;
    let name = wide(&value_name());
    let result = if enabled {
        let command = current_command()?;
        let bytes = command.len() * std::mem::size_of::<u16>();
        let status = unsafe {
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                command.as_ptr() as *const u8,
                bytes as u32,
            )
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("写入自启项失败（错误 {status}）"))
        }
    } else {
        let status = unsafe { RegDeleteValueW(key, name.as_ptr()) };
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("删除自启项失败（错误 {status}）"))
        }
    };
    unsafe { RegCloseKey(key) };
    result
}

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        set_enabled(enabled)
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err("当前平台暂未实现开机自启".to_string())
    }
}

#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    false
}
