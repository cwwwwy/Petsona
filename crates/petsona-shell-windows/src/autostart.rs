use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME_ENV: &str = "PETSONA_AUTOSTART_VALUE_NAME";

fn value_name() -> String {
    std::env::var(VALUE_NAME_ENV)
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Petsona".to_string())
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Read the login item straight from the registry: the settings checkbox
/// shows the real OS state, not a cached config copy.
pub(crate) fn is_enabled() -> bool {
    let subkey = wide(RUN_KEY);
    let name = wide(&value_name());
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut key) != ERROR_SUCCESS
        {
            return false;
        }
        let mut kind = 0u32;
        let mut size = 0u32;
        let status = RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null(),
            &mut kind,
            std::ptr::null_mut(),
            &mut size,
        );
        RegCloseKey(key);
        status == ERROR_SUCCESS
    }
}

pub(crate) fn enable() -> Result<(), String> {
    let command = command_line()?;
    let subkey = wide(RUN_KEY);
    let name = wide(&value_name());
    let data: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let status = RegCreateKeyW(HKEY_CURRENT_USER, subkey.as_ptr(), &mut key);
        if status != ERROR_SUCCESS {
            return Err(format!("无法创建开机自启注册表项（错误码 {status}）"));
        }
        let status = RegSetValueExW(
            key,
            name.as_ptr(),
            0,
            REG_SZ,
            data.as_ptr() as *const u8,
            (data.len() * std::mem::size_of::<u16>()) as u32,
        );
        RegCloseKey(key);
        if status != ERROR_SUCCESS {
            return Err(format!("无法写入开机自启注册表项（错误码 {status}）"));
        }
    }
    Ok(())
}

pub(crate) fn disable() -> Result<(), String> {
    let subkey = wide(RUN_KEY);
    let name = wide(&value_name());
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        ) != ERROR_SUCCESS
        {
            // No Run key means there is nothing to remove.
            return Ok(());
        }
        let status = RegDeleteValueW(key, name.as_ptr());
        RegCloseKey(key);
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("无法删除开机自启注册表项（错误码 {status}）"))
        }
    }
}

fn command_line() -> Result<String, String> {
    let exe =
        std::env::current_exe().map_err(|error| format!("无法读取当前可执行文件路径：{error}"))?;
    Ok(format!("\"{}\"", exe.display()))
}
