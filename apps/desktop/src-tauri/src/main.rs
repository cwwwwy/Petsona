#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod logging;
#[cfg(windows)]
mod overlay;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

fn main() {
    logging::mark_start();
    logging::log("main: start");

    // Dev/test flag: open the settings window immediately (used by M0 checks
    // to verify foreground focus without driving the tray menu).
    let show_settings = std::env::args().any(|arg| arg == "--show-settings");

    #[cfg(windows)]
    overlay::spawn();

    tauri::Builder::default()
        .setup(move |app| {
            let show = MenuItem::with_id(app, "show", "设置…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            let icon = tauri::image::Image::from_bytes(include_bytes!(
                "../../../../packaging/windows/Petsona.ico"
            ))?;

            TrayIconBuilder::with_id("petsona-tray")
                .icon(icon)
                .tooltip("Petsona")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("settings") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            if show_settings {
                if let Some(window) = app.get_webview_window("settings") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    logging::log("tauri: settings window shown (--show-settings)");
                }
            }

            logging::log("tauri: setup complete (tray ready)");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Petsona desktop shell");
}
