#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod dialog;
#[cfg(windows)]
mod gdi_text;
mod logging;
#[cfg(windows)]
mod overlay;
mod settings;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use petsona_runtime::commands::RuntimeCommand;
use petsona_runtime::engine::RuntimeEngine;
use petsona_runtime::snapshot::RuntimeTextField;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

/// The shell owns the runtime handle; the worker thread owns all disk and
/// protocol state (same ownership split the old FFI exposed).
pub(crate) type Engine = Arc<Mutex<RuntimeEngine>>;

fn main() {
    logging::mark_start();
    logging::log("main: start");

    let args: Vec<String> = std::env::args().collect();
    let show_settings = args.iter().any(|arg| arg == "--show-settings");
    let show_chat = args.iter().any(|arg| arg == "--show-chat");
    // Dev/test flag: open the composer shortly after startup (used by the
    // mouse-free smoke to verify the native EDIT control end to end).
    let open_composer = args.iter().any(|arg| arg == "--open-composer");
    let exit_after_ms = args
        .iter()
        .position(|arg| arg == "--exit-after-ms")
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse::<u64>().ok());

    // The runtime worker resolves PETSONA_HOME (or the platform data dir),
    // acquires petsona.lock, loads config/pets and binds the state protocol.
    // A second instance fails the lock and surfaces as a faulted snapshot.
    let engine: Engine = match RuntimeEngine::spawn(None, || {}) {
        Ok(engine) => Arc::new(Mutex::new(engine)),
        Err(error) => {
            logging::log(&format!("main: cannot spawn the runtime worker: {error:#}"));
            return;
        }
    };
    logging::log("main: runtime worker spawned");

    #[cfg(windows)]
    overlay::spawn(Arc::clone(&engine));

    let engine_for_setup = Arc::clone(&engine);
    tauri::Builder::default()
        .manage(Arc::clone(&engine))
        .invoke_handler(tauri::generate_handler![
            settings::settings_snapshot,
            settings::settings_action,
            settings::open_data_path,
            settings::open_external_url,
            settings::open_chat_window,
            settings::pet_preview,
            dialog::pick_import_zip,
            dialog::pick_import_folder,
            dialog::pick_export_zip,
            dialog::pick_persona_import,
            dialog::pick_persona_export,
            dialog::pick_persona_source,
            dialog::pick_memory_import,
            dialog::pick_memory_export,
            autostart::set_autostart,
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // Closing the settings window hides it; the shell keeps running
                // with its pet and tray until the user quits from the tray.
                api.prevent_close();
                let _ = window.hide();
            }
            tauri::WindowEvent::ThemeChanged(theme) => {
                // Re-apply "follow system" so the native title bar redraws in
                // the same event loop turn as the WebView's color-scheme.
                let _ = window.set_theme(None);
                #[cfg(windows)]
                overlay::set_dark_theme(matches!(theme, tauri::Theme::Dark));
            }
            _ => {}
        })
        .setup(move |app| {
            let dark = app
                .get_webview_window("settings")
                .and_then(|window| window.theme().ok())
                .map(|theme| matches!(theme, tauri::Theme::Dark))
                .unwrap_or(false);
            #[cfg(windows)]
            overlay::set_dark_theme(dark);

            build_tray(app, Arc::clone(&engine_for_setup))?;

            if show_settings {
                show_settings_window(app.handle());
                logging::log("tauri: settings window shown (--show-settings)");
            }

            if show_chat {
                show_chat_window(app.handle());
                logging::log("tauri: chat window shown (--show-chat)");
            }

            if open_composer {
                thread::spawn(|| {
                    for _ in 0..8 {
                        thread::sleep(Duration::from_millis(700));
                        #[cfg(windows)]
                        overlay::request_open_composer();
                    }
                });
            }

            start_watcher(app.handle().clone(), Arc::clone(&engine_for_setup));

            if let Some(milliseconds) = exit_after_ms {
                let handle = app.handle().clone();
                let engine = Arc::clone(&engine_for_setup);
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(milliseconds));
                    stop_and_exit(&handle, &engine);
                });
            }

            logging::log("tauri: setup complete (tray ready)");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Petsona desktop shell");
}

fn build_tray(app: &mut tauri::App, engine: Engine) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "设置…", true, None::<&str>)?;
    let chat = MenuItem::with_id(app, "chat", "聊天与历史", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏宠物", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &chat, &toggle, &separator, &quit])?;

    let icon = tauri::image::Image::from_bytes(include_bytes!(
        "../../../../packaging/windows/Petsona.ico"
    ))?;

    let engine_for_menu = Arc::clone(&engine);
    TrayIconBuilder::with_id("petsona-tray")
        .icon(icon)
        .tooltip("Petsona")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => show_settings_window(app),
            "chat" => show_chat_window(app),
            "toggle" => toggle_pet_visibility(&engine_for_menu),
            "quit" => stop_and_exit(app, &engine_for_menu),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// Presents a hidden or minimized content window without recreating it, so a
/// tray menu action always returns the user to a focused, usable window.
fn present_window<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn show_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        present_window(&window);
    }
}

pub(crate) fn show_chat_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("chat") {
        present_window(&window);
    }
}

fn toggle_pet_visibility(engine: &Engine) {
    if let Ok(engine) = engine.lock() {
        let visible = engine.snapshot().pet_visible;
        let _ = engine.send(RuntimeCommand::SetVisibility(!visible));
        logging::log(&format!("tray: pet visibility -> {}", !visible));
    }
}

fn stop_and_exit(app: &AppHandle, engine: &Engine) {
    logging::log("shell: quit requested");
    if let Ok(mut engine) = engine.lock() {
        engine.stop();
    }
    app.exit(0);
}

/// Watches the runtime snapshot: a faulted worker (second instance / fatal
/// error) must not leave an inert process behind, and an empty pet library
/// opens the settings window once, matching the old frontends.
fn start_watcher(app: AppHandle, engine: Engine) {
    thread::spawn(move || {
        logging::log("watcher: started");
        let mut settings_presented = false;
        let mut fault_logged = false;
        loop {
            thread::sleep(Duration::from_millis(200));
            let (faulted, error, ready, has_pet) = {
                let Ok(engine) = engine.lock() else {
                    return;
                };
                let snapshot = engine.snapshot();
                (
                    snapshot.faulted,
                    engine.text(RuntimeTextField::Error),
                    snapshot.ready,
                    snapshot.has_pet,
                )
            };

            if faulted {
                if !fault_logged {
                    fault_logged = true;
                    if error.to_lowercase().contains("lock") {
                        logging::log(
                            "watcher: another instance owns the data directory; the overlay shows a notice and exits shortly",
                        );
                    } else {
                        logging::log(&format!("watcher: runtime faulted: {error}"));
                    }
                }
                // The overlay renders the fault bubble (and exits after ~3 s on
                // a lock conflict); the watcher only keeps a record.
                continue;
            }

            if ready && !has_pet && !settings_presented {
                settings_presented = true;
                logging::log("watcher: no pet loaded; opening the settings window");
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || show_settings_window(&handle));
            }
        }
    });
}
