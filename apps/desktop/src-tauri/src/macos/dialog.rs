//! Existing IPC names return a selected path or null, never AppKit objects.
use std::cell::Cell;
use std::sync::mpsc;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSModalResponseOK, NSOpenPanel, NSSavePanel};
use objc2_foundation::{NSArray, NSString};
use tauri::AppHandle;

thread_local! {
    static PANEL_OPEN: Cell<bool> = const { Cell::new(false) };
}

struct PanelGuard;
impl Drop for PanelGuard {
    fn drop(&mut self) {
        PANEL_OPEN.with(|open| open.set(false));
    }
}

pub enum Choice {
    File {
        title: &'static str,
        extensions: &'static [&'static str],
    },
    Folder,
    Save {
        title: &'static str,
        name: String,
        extension: &'static str,
    },
}

/// AppKit extension filtering predates UTType but remains supported. Keeping
/// this narrow deprecated call avoids adding an otherwise unused framework.
#[allow(deprecated)]
fn choose(choice: Choice) -> Result<Option<String>, String> {
    let main = MainThreadMarker::new().ok_or("文件对话框必须在主线程打开")?;
    if PANEL_OPEN.with(|open| open.replace(true)) {
        return Err("已有文件对话框打开，请先完成或取消它".into());
    }
    let _guard = PanelGuard;
    match choice {
        Choice::File { title, extensions } => {
            let panel = NSOpenPanel::openPanel(main);
            panel.setTitle(Some(&NSString::from_str(title)));
            panel.setCanChooseFiles(true);
            panel.setCanChooseDirectories(false);
            panel.setAllowsMultipleSelection(false);
            let types: Vec<_> = extensions
                .iter()
                .map(|ext| NSString::from_str(ext))
                .collect();
            panel.setAllowedFileTypes(Some(&NSArray::from_retained_slice(&types)));
            if panel.runModal() != NSModalResponseOK {
                return Ok(None);
            }
            Ok(panel
                .URL()
                .and_then(|url| url.path())
                .map(|path| path.to_string()))
        }
        Choice::Folder => {
            let panel = NSOpenPanel::openPanel(main);
            panel.setTitle(Some(&NSString::from_str("选择宠物文件夹")));
            panel.setCanChooseFiles(false);
            panel.setCanChooseDirectories(true);
            panel.setAllowsMultipleSelection(false);
            if panel.runModal() != NSModalResponseOK {
                return Ok(None);
            }
            Ok(panel
                .URL()
                .and_then(|url| url.path())
                .map(|path| path.to_string()))
        }
        Choice::Save {
            title,
            name,
            extension,
        } => {
            let panel = NSSavePanel::savePanel(main);
            panel.setTitle(Some(&NSString::from_str(title)));
            panel.setCanCreateDirectories(true);
            panel.setAllowsOtherFileTypes(false);
            panel.setAllowedFileTypes(Some(&NSArray::from_retained_slice(&[NSString::from_str(
                extension,
            )])));
            panel.setNameFieldStringValue(&NSString::from_str(&name));
            if panel.runModal() != NSModalResponseOK {
                return Ok(None);
            }
            Ok(panel
                .URL()
                .and_then(|url| url.path())
                .map(|path| path.to_string()))
        }
    }
}

pub async fn pick(app: AppHandle, choice: Choice) -> Result<Option<String>, String> {
    let (sender, receiver) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = sender.send(choose(choice));
    })
    .map_err(|error| format!("无法打开文件对话框：{error}"))?;
    // The waiting task owns only strings. Never block the Tauri main thread
    // while dispatching to that same thread, and never hold the engine mutex.
    tauri::async_runtime::spawn_blocking(move || receiver.recv())
        .await
        .map_err(|error| format!("文件对话框任务失败：{error}"))?
        .map_err(|_| "文件对话框已关闭".to_string())?
}
