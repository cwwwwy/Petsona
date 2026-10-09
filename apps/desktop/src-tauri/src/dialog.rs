//! Native Windows file/folder pickers used by the settings UI.
//!
//! These are intentionally thin wrappers around the classic common dialogs:
//! the settings module owns all business actions, and this module only turns
//! a user choice into an absolute path.

#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::ptr::null;

#[cfg(windows)]
use windows_sys::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, COINIT_APARTMENTTHREADED};
#[cfg(windows)]
use windows_sys::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY,
    OFN_NOCHANGEDIR, OFN_NOREADONLYRETURN, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
#[cfg(windows)]
use windows_sys::Win32::UI::Shell::{
    SHBrowseForFolderW, SHGetPathFromIDListW, BIF_EDITBOX, BIF_NEWDIALOGSTYLE,
    BIF_RETURNONLYFSDIRS, BROWSEINFOW,
};
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

/// Allocates the zero-filled buffer the classic file dialogs write into. The
/// buffer is pre-seeded with `initial` so the dialog opens on a sensible name,
/// while its *full* length stays available for the path the dialog returns.
#[cfg(windows)]
fn dialog_buffer(initial: &str, capacity: usize) -> Vec<u16> {
    let mut buffer = vec![0u16; capacity.max(1)];
    for (index, unit) in OsStr::new(initial).encode_wide().enumerate() {
        if index + 1 >= buffer.len() {
            break;
        }
        buffer[index] = unit;
    }
    buffer
}

/// Reads the NUL-terminated path the dialog wrote into the buffer. The scan
/// must cover the whole allocated buffer: the returned absolute path is longer
/// than the default name the buffer was seeded with.
#[cfg(windows)]
fn path_from_buffer(buffer: &[u16]) -> Option<String> {
    let length = buffer.iter().position(|unit| *unit == 0)?;
    if length == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..length]))
}

#[cfg(windows)]
fn settings_owner() -> windows_sys::Win32::Foundation::HWND {
    let class = wide("Tauri Window");
    unsafe { FindWindowW(class.as_ptr(), null()) }
}

#[cfg(windows)]
fn initialize_com() {
    // The common dialogs can be called without COM, but the shell folder
    // picker may create shell COM objects. Ignoring RPC_E_CHANGED_MODE is
    // intentional: another runtime thread may already own a different mode.
    unsafe {
        let _ = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
    }
}

#[cfg(windows)]
fn open_file_dialog(title: &str, filter: &str, default_extension: &str) -> Option<String> {
    initialize_com();
    let mut buffer = dialog_buffer("", 32_768);
    let filter = wide(filter);
    let title = wide(title);
    let default_extension = wide(default_extension);
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: settings_owner(),
        lpstrFilter: filter.as_ptr(),
        lpstrFile: buffer.as_mut_ptr(),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: title.as_ptr(),
        lpstrDefExt: default_extension.as_ptr(),
        Flags: OFN_EXPLORER
            | OFN_FILEMUSTEXIST
            | OFN_PATHMUSTEXIST
            | OFN_NOCHANGEDIR
            | OFN_HIDEREADONLY,
        ..OPENFILENAMEW::default()
    };

    let accepted = unsafe { GetOpenFileNameW(&mut dialog) } != 0;
    if !accepted {
        return None;
    }
    path_from_buffer(&buffer)
}

#[cfg(windows)]
fn save_file_dialog(
    title: &str,
    default_name: &str,
    filter_text: &str,
    default_extension: &str,
) -> Option<String> {
    initialize_com();
    // Regression: the dialog writes the chosen absolute path over the buffer
    // (`C:\...\export.zip`), so the buffer must be zero-filled at full
    // length and the result scanned across its whole length. Seeding a short
    // Vec and only growing its capacity made `buffer.iter()` stop at the
    // default name, so the returned path was read as the default name or as
    // an empty string and the export silently did nothing.
    let mut buffer = dialog_buffer(default_name, 32_768);
    let filter = wide(filter_text);
    let title = wide(title);
    let default_extension = wide(default_extension);
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: settings_owner(),
        lpstrFilter: filter.as_ptr(),
        lpstrFile: buffer.as_mut_ptr(),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: title.as_ptr(),
        lpstrDefExt: default_extension.as_ptr(),
        Flags: OFN_EXPLORER
            | OFN_PATHMUSTEXIST
            | OFN_OVERWRITEPROMPT
            | OFN_NOREADONLYRETURN
            | OFN_NOCHANGEDIR,
        ..OPENFILENAMEW::default()
    };

    let accepted = unsafe { GetSaveFileNameW(&mut dialog) } != 0;
    if !accepted {
        return None;
    }
    path_from_buffer(&buffer)
}

#[cfg(windows)]
fn pick_folder(title: &str) -> Option<String> {
    initialize_com();
    let title = wide(title);
    let mut display_name = vec![0u16; 512];
    let browse = BROWSEINFOW {
        hwndOwner: settings_owner(),
        pszDisplayName: display_name.as_mut_ptr(),
        lpszTitle: title.as_ptr(),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX,
        ..BROWSEINFOW::default()
    };

    let pidl = unsafe { SHBrowseForFolderW(&browse) };
    if pidl.is_null() {
        return None;
    }
    let mut path = vec![0u16; 32_768];
    let result = unsafe { SHGetPathFromIDListW(pidl, path.as_mut_ptr()) } != 0;
    unsafe { CoTaskMemFree(pidl as *mut _) };
    if !result {
        return None;
    }
    let length = path.iter().position(|unit| *unit == 0).unwrap_or(0);
    if length == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&path[..length]))
}

#[tauri::command]
pub fn pick_import_zip() -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        Ok(open_file_dialog(
            "选择宠物 ZIP 压缩包",
            "宠物压缩包 (*.zip)\0*.zip\0所有文件 (*.*)\0*.*\0\0",
            "zip",
        ))
    }
    #[cfg(not(windows))]
    {
        Err("当前平台暂未实现原生文件选择".to_string())
    }
}

#[tauri::command]
pub fn pick_import_folder() -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        Ok(pick_folder("选择宠物文件夹"))
    }
    #[cfg(not(windows))]
    {
        Err("当前平台暂未实现原生文件夹选择".to_string())
    }
}

#[tauri::command]
pub fn pick_persona_import() -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        Ok(open_file_dialog(
            "导入人格 JSON",
            "人格文件 (*.json)\0*.json\0所有文件 (*.*)\0*.*\0\0",
            "json",
        ))
    }
    #[cfg(not(windows))]
    {
        Err("当前平台暂未实现原生文件选择".to_string())
    }
}

#[tauri::command]
pub fn pick_persona_export(default_name: String) -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        let name = if default_name.to_ascii_lowercase().ends_with(".json") {
            default_name
        } else {
            format!("{default_name}.json")
        };
        Ok(save_file_dialog(
            "导出人格 JSON",
            &name,
            "人格文件 (*.json)\0*.json\0所有文件 (*.*)\0*.*\0\0",
            "json",
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = default_name;
        Err("当前平台暂未实现原生保存选择".to_string())
    }
}

#[tauri::command]
pub fn pick_persona_source() -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        Ok(open_file_dialog(
            "选择人格资料",
            "文本或 JSON (*.txt;*.json)\0*.txt;*.json\0所有文件 (*.*)\0*.*\0\0",
            "txt",
        ))
    }
    #[cfg(not(windows))]
    {
        Err("当前平台暂未实现原生文件选择".to_string())
    }
}

#[tauri::command]
pub fn pick_memory_import() -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        Ok(open_file_dialog(
            "导入记忆 JSON",
            "记忆文件 (*.json)\0*.json\0所有文件 (*.*)\0*.*\0\0",
            "json",
        ))
    }
    #[cfg(not(windows))]
    {
        Err("当前平台暂未实现原生文件选择".to_string())
    }
}

#[tauri::command]
pub fn pick_memory_export(default_name: String) -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        let name = if default_name.to_ascii_lowercase().ends_with(".json") {
            default_name
        } else {
            format!("{default_name}.json")
        };
        Ok(save_file_dialog(
            "导出记忆 JSON",
            &name,
            "记忆文件 (*.json)\0*.json\0所有文件 (*.*)\0*.*\0\0",
            "json",
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = default_name;
        Err("当前平台暂未实现原生保存选择".to_string())
    }
}

#[tauri::command]
pub fn pick_export_zip(default_name: String) -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        let name = if default_name.to_ascii_lowercase().ends_with(".zip") {
            default_name
        } else {
            format!("{default_name}.zip")
        };
        Ok(save_file_dialog(
            "导出宠物",
            &name,
            "宠物压缩包 (*.zip)\0*.zip\0所有文件 (*.*)\0*.*\0\0",
            "zip",
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = default_name;
        Err("当前平台暂未实现原生保存选择".to_string())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::{dialog_buffer, path_from_buffer};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    /// The dialog returns an absolute path that is longer than the default
    /// name; the reader must see the whole buffer, not just the seeded prefix.
    #[test]
    fn dialog_buffer_reads_a_path_longer_than_the_default_name() {
        let path = r"C:\Users\tester\AppData\Local\Temp\petsona-fa2\export-rocky.zip";
        let mut buffer = dialog_buffer("rocky.zip", 96);
        for (index, unit) in OsStr::new(path).encode_wide().chain(Some(0)).enumerate() {
            buffer[index] = unit;
        }
        assert_eq!(path_from_buffer(&buffer).as_deref(), Some(path));
    }

    #[test]
    fn dialog_buffer_keeps_the_default_name_zero_terminated() {
        let buffer = dialog_buffer("rocky.zip", 64);
        assert_eq!(path_from_buffer(&buffer).as_deref(), Some("rocky.zip"));
    }

    #[test]
    fn dialog_buffer_truncates_a_default_name_that_does_not_fit() {
        let buffer = dialog_buffer("a-default-name-that-does-not-fit.zip", 8);
        assert_eq!(buffer.len(), 8);
        assert_eq!(path_from_buffer(&buffer).as_deref(), Some("a-defau"));
    }

    #[test]
    fn empty_dialog_buffer_returns_none() {
        let buffer = dialog_buffer("", 64);
        assert_eq!(path_from_buffer(&buffer), None);
    }
}
