//! Petsona shared UI crate.
//!
//! This crate owns the egui UI, the portable application wiring and the
//! [`platform::PlatformHost`] boundary. Native window, input and menu behaviour
//! is provided by the platform shells (`petsona-shell-windows`,
//! `petsona-shell-macos`).

pub mod app;
mod fonts;
pub mod platform;
#[cfg(feature = "test-hooks")]
mod test_hooks;

use std::sync::Arc;

use petsona_core::config::{AppConfig, AppPaths};
use petsona_runtime::{instance_lock, logging};
use platform::PlatformHost;

pub type RunResult = eframe::Result;

/// Run the shared application with a platform backend.
pub fn run(host: Arc<dyn PlatformHost>) -> RunResult {
    let paths = AppPaths::default();
    if let Err(error) = paths.ensure() {
        eprintln!("cannot prepare Petsona data directory: {error}");
        return Ok(());
    }
    logging::init(&paths.logs_dir);
    tracing::info!(host = host.name(), "starting Petsona");

    let _instance_lock =
        match instance_lock::InstanceLock::acquire(&paths.config_dir.join("petsona.lock")) {
            Ok(lock) => lock,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                tracing::warn!(
                    path = %paths.config_dir.display(),
                    "Petsona is already running",
                );
                return Ok(());
            }
            Err(error) => {
                tracing::error!(%error, "cannot acquire Petsona instance lock");
                return Ok(());
            }
        };

    let config = AppConfig::load(&paths.config_file).unwrap_or_default();
    let app = match app::PetsonaApp::new(paths, config, host) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("Petsona failed to start: {error:#}");
            return Ok(());
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Petsona")
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
        "Petsona",
        options,
        Box::new(move |creation_context| {
            let mut app = app;
            app.initialize(creation_context);
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    )
}
