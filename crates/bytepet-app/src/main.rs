#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod fonts;
mod greeting;
mod instance_lock;
mod logging;
mod platform;

use bytepet_core::config::{AppConfig, AppPaths};

fn main() -> eframe::Result {
    let paths = AppPaths::default();
    if let Err(error) = paths.ensure() {
        eprintln!("cannot prepare BytePet data directory: {error}");
        return Ok(());
    }
    logging::init(&paths.logs_dir);

    let _instance_lock =
        match instance_lock::InstanceLock::acquire(&paths.config_dir.join("bytepet.lock")) {
            Ok(lock) => lock,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                tracing::warn!(
                    path = %paths.config_dir.display(),
                    "BytePet is already running",
                );
                return Ok(());
            }
            Err(error) => {
                tracing::error!(%error, "cannot acquire BytePet instance lock");
                return Ok(());
            }
        };

    let config = AppConfig::load(&paths.config_file).unwrap_or_default();
    let app = match app::BytePetApp::new(paths, config) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("BytePet failed to start: {error:#}");
            return Ok(());
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("BytePet")
            .with_inner_size([220.0, 280.0])
            .with_min_inner_size([140.0, 180.0])
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_active(false)
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        "BytePet",
        options,
        Box::new(move |creation_context| {
            let mut app = app;
            app.initialize(creation_context);
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    )
}
