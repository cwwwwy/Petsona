//! Petsona Windows shell.
//!
//! The shell owns the Windows entry point and every Win32 call; the shared egui
//! UI, the runtime and the pet engine live in `petsona-app`, `petsona-runtime`
//! and `petsona-core`.

#[cfg(target_os = "windows")]
mod autostart;
#[cfg(target_os = "windows")]
mod menu;
#[cfg(target_os = "windows")]
mod no_activate;
#[cfg(target_os = "windows")]
mod platform;

#[cfg(target_os = "windows")]
pub use platform::WindowsHost;

/// Run the Windows shell.
#[cfg(target_os = "windows")]
pub fn run() -> petsona_app::RunResult {
    let host = std::sync::Arc::new(WindowsHost::new());
    petsona_app::run(host)
}

/// Other hosts can still build the workspace; this binary is Windows-only.
#[cfg(not(target_os = "windows"))]
pub fn run() -> petsona_app::RunResult {
    eprintln!("petsona-shell-windows is intended for its native platform");
    Ok(())
}
