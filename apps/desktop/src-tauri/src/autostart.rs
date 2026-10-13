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

// Pure command construction is testable on Mac without touching Windows Run.
#[cfg(any(windows, test))]
fn login_command(
    executable: &str,
    value: &str,
    launcher: Option<&str>,
    shell: &str,
    isolated: bool,
) -> Result<String, String> {
    fn quoted(value: &str) -> Result<String, String> {
        if value.is_empty() || value.chars().any(|c| c.is_control() || c == '"') {
            return Err("自启路径含不支持的字符".into());
        }
        Ok(format!("\"{value}\""))
    }
    let command = match launcher {
        None => quoted(executable)?,
        Some(path) => {
            let absolute = path.starts_with("\\\\")
                || (path.as_bytes().get(1) == Some(&b':')
                    && path.as_bytes().get(2) == Some(&b'\\')
                    && path.as_bytes()[0].is_ascii_alphabetic());
            if !isolated
                || !value.starts_with("Petsona.Accept.")
                || value.len() <= "Petsona.Accept.".len()
                || !absolute
                || !path.to_ascii_lowercase().ends_with(".ps1")
            {
                return Err("登录验收启动脚本必须使用专用数据、凭据和自启值名以及绝对路径".into());
            }
            format!(
                "{} -NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File {}",
                quoted(shell)?,
                quoted(path)?
            )
        }
    };
    if command.encode_utf16().count() > 260 {
        return Err("自启命令超过 Windows Run 的 260 字符限制，请缩短应用或验收目录路径".into());
    }
    Ok(command)
}

#[cfg(windows)]
fn current_command() -> Result<Vec<u16>, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("无法读取应用路径：{error}"))?;
    #[cfg(feature = "acceptance-hooks")]
    let launcher = std::env::var("PETSONA_AUTOSTART_LAUNCHER").ok();
    #[cfg(not(feature = "acceptance-hooks"))]
    let launcher: Option<String> = {
        if std::env::var_os("PETSONA_AUTOSTART_LAUNCHER").is_some() {
            return Err("该构建不包含登录验收入口".into());
        }
        None
    };
    if let Some(path) = &launcher {
        if !std::path::Path::new(path).is_file() {
            return Err("登录验收启动脚本不存在".into());
        }
    }
    let shell = std::env::var("SystemRoot")
        .map(|root| format!("{root}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"))
        .unwrap_or_default();
    let isolated = std::env::var_os("PETSONA_HOME").is_some_and(|home| !home.is_empty())
        && std::env::var("PETSONA_CREDENTIAL_SERVICE")
            .is_ok_and(|service| service.starts_with("com.petsona.accept."));
    let command = login_command(
        &executable.to_string_lossy(),
        &value_name(),
        launcher.as_deref(),
        &shell,
        isolated,
    )?;
    Ok(wide(&command))
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
    // Construct before opening the registry handle so validation errors cannot leak it.
    let command = if enabled {
        Some(current_command()?)
    } else {
        None
    };
    let key = create_run_key()?;
    let name = wide(&value_name());
    let result = if let Some(command) = command {
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
    #[cfg(target_os = "macos")]
    {
        crate::macos::autostart::set_enabled(enabled)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = enabled;
        Err("当前平台暂未实现开机自启".to_string())
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn is_enabled() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub fn is_enabled() -> bool {
    crate::macos::autostart::is_enabled()
}

#[cfg(test)]
mod command_tests {
    use super::login_command;

    #[test]
    fn ordinary_login_keeps_direct_executable_and_isolation_is_required_for_launcher() {
        let exe = r"C:\宠物 伙伴\petsona-desktop.exe";
        let launcher = r"C:\Users\tester\petsona-rg\session\launch.ps1";
        let shell = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
        assert_eq!(
            login_command(exe, "Petsona", None, shell, false).unwrap(),
            format!("\"{exe}\"")
        );
        assert!(login_command(exe, "Petsona", Some(launcher), shell, true).is_err());
        assert!(login_command(exe, "Petsona.Accept.id", Some(launcher), shell, false).is_err());
        let command = login_command(exe, "Petsona.Accept.id", Some(launcher), shell, true).unwrap();
        assert!(command.ends_with(&format!("-File \"{launcher}\"")));
        assert!(command.starts_with(&format!("\"{shell}\"")));
    }

    #[test]
    fn login_rejects_relative_paths_quotes_controls_and_run_overflow() {
        for path in [
            "relative.ps1",
            "C:\\bad\".ps1",
            "C:\\bad\n.ps1",
            "C:\\bad.exe",
        ] {
            assert!(login_command(
                "app.exe",
                "Petsona.Accept.id",
                Some(path),
                "powershell.exe",
                true
            )
            .is_err());
        }
        assert!(login_command(&"a".repeat(261), "Petsona", None, "", false).is_err());
    }
}
