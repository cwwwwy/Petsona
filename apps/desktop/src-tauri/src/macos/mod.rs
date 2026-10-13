//! AppKit objects stay on the Tauri main thread; runtime work stays off it.
pub mod autostart;
pub mod bubble;
pub mod composer;
pub mod dialog;
pub mod geometry;
pub mod overlay;

use std::cell::Cell;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use tauri::{AppHandle, Manager};

thread_local! {
    static DOCK_VISIBLE: Cell<Option<bool>> = const { Cell::new(None) };
}

fn set_dock_visible(visible: bool) {
    let Some(main) = MainThreadMarker::new() else {
        crate::logging::log("macos: refused activation policy off the main thread");
        return;
    };
    DOCK_VISIBLE.with(|cached| {
        if cached.get() == Some(visible) {
            return;
        }
        let policy = if visible {
            NSApplicationActivationPolicy::Regular
        } else {
            NSApplicationActivationPolicy::Accessory
        };
        if NSApplication::sharedApplication(main).setActivationPolicy(policy) {
            cached.set(Some(visible));
            crate::logging::log(&format!("macos: dock visible={visible}"));
        } else {
            crate::logging::log("macos: activation policy was rejected");
        }
    });
}

/// Hide Dock only after the last content window has been hidden.
pub fn sync_dock(app: &AppHandle) {
    let handle = app.clone();
    let update = move || {
        let visible = ["settings", "chat"].iter().any(|label| {
            handle
                .get_webview_window(label)
                .is_some_and(|window| window.is_visible().unwrap_or(false))
        });
        set_dock_visible(visible);
    };
    if MainThreadMarker::new().is_some() {
        update();
    } else if let Err(error) = app.run_on_main_thread(update) {
        crate::logging::log(&format!("macos: cannot update Dock: {error}"));
    }
}

pub fn present_window(app: &AppHandle, label: &str) {
    let handle = app.clone();
    let label = label.to_string();
    let show = move || {
        if let Some(window) = handle.get_webview_window(&label) {
            set_dock_visible(true);
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
            if let Some(main) = MainThreadMarker::new() {
                NSApplication::sharedApplication(main).activate();
            }
            crate::logging::log(&format!("macos: content shown={label}"));
        }
    };
    if MainThreadMarker::new().is_some() {
        show();
    } else if let Err(error) = app.run_on_main_thread(show) {
        crate::logging::log(&format!("macos: cannot present content: {error}"));
    }
}

/// Acceptance hook operates on the application's own windows, never injects
/// keyboard/mouse events. Compare actual AppKit policy and native visibility.
#[cfg(feature = "acceptance-hooks")]
pub fn verify_content_lifecycle(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let steps = [
            ("settings", false, false, false),
            ("settings", true, true, false),
            ("chat", true, true, true),
            ("settings", false, false, true),
            ("chat", false, false, false),
            ("settings", true, true, false),
        ];
        for (label, show, settings_visible, chat_visible) in steps {
            let handle = app.clone();
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            if app.run_on_main_thread(move || {
                if show { present_window(&handle, label); }
                else if let Some(window) = handle.get_webview_window(label) {
                    let _ = window.hide();
                    sync_dock(&handle);
                }
                let Some(main) = MainThreadMarker::new() else { let _ = sender.send(false); return; };
                let actual_settings = handle.get_webview_window("settings").is_some_and(|w| w.is_visible().unwrap_or(false));
                let actual_chat = handle.get_webview_window("chat").is_some_and(|w| w.is_visible().unwrap_or(false));
                let expected_policy = if settings_visible || chat_visible { NSApplicationActivationPolicy::Regular } else { NSApplicationActivationPolicy::Accessory };
                let policy = NSApplication::sharedApplication(main).activationPolicy();
                let passed = actual_settings == settings_visible && actual_chat == chat_visible && policy == expected_policy;
                crate::logging::log(&format!("macos: lifecycle step={label}/{show} settings={actual_settings} chat={actual_chat} dock={} pass={passed}", policy == NSApplicationActivationPolicy::Regular));
                let _ = sender.send(passed);
            }).is_err() || receiver.recv_timeout(std::time::Duration::from_secs(4)).ok() != Some(true) {
                crate::logging::log("macos: lifecycle FAIL");
                return;
            }
        }
        crate::logging::log("macos: lifecycle PASS");
    });
}
