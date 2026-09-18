//! Petsona macOS shell.
//!
//! The shell owns the macOS entry point and every AppKit / NSEvent /
//! CoreGraphics call; the shared egui UI, the runtime and the pet engine live
//! in `petsona-app`, `petsona-runtime` and `petsona-core`.

#[cfg(target_os = "macos")]
mod autostart;

#[cfg(target_os = "macos")]
mod platform;

#[cfg(target_os = "macos")]
pub use platform::MacHost;

/// Run the macOS shell.
#[cfg(target_os = "macos")]
pub fn run() -> petsona_app::RunResult {
    let host = std::sync::Arc::new(MacHost::new());
    petsona_app::run(host)
}

/// Other hosts can still build the workspace; this binary is macOS-only.
#[cfg(not(target_os = "macos"))]
pub fn run() -> petsona_app::RunResult {
    eprintln!("petsona-shell-macos is intended for its native platform");
    Ok(())
}
