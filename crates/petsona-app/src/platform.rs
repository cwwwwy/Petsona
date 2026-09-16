//! Platform host boundary.
//!
//! `petsona-app` owns the shared egui UI and the portable runtime wiring. Every
//! call that has to touch the operating system goes through [`PlatformHost`],
//! which is implemented by the platform shells:
//!
//! - `petsona-shell-windows`: Win32 window chrome, `WH_MOUSE_LL` input hook and
//!   the popup-menu thread.
//! - `petsona-shell-macos`: AppKit window masks, NSEvent/CoreGraphics input and
//!   the tray-icon context menu.
//!
//! Every method has a portable default, so this crate never needs a
//! `#[cfg(target_os = ...)]` of its own: a shell that leaves a method alone
//! falls back to winit's and egui's own behaviour.

use std::path::{Path, PathBuf};

use eframe::egui;

/// Global pointer state sampled from the system instead of from egui events.
#[derive(Debug, Clone, Copy, Default)]
pub struct PointerSnapshot {
    pub position: Option<(f64, f64)>,
    pub primary_down: Option<bool>,
    pub secondary_down: Option<bool>,
}

/// What a native context menu asks the shared UI to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    OpenSettings,
    ChangePet,
    TogglePet,
    Quit,
}

/// A context menu owned by a platform shell.
pub trait PlatformMenu: Send {
    /// Pop up the menu at a physical screen position.
    fn show(&self, x: f64, y: f64, pet_visible: bool);

    /// Close the menu if it is open. The default is a no-op because some
    /// backends (macOS AppKit menus, egui) own their own dismissal.
    fn dismiss(&self) {}

    /// Commands the user selected since the last call.
    fn poll(&self) -> Vec<MenuCommand>;
}

/// Everything the shared UI needs from the operating system.
pub trait PlatformHost: Send + Sync + 'static {
    /// Short name, used in logs.
    fn name(&self) -> &'static str {
        "portable"
    }

    /// Called with the pet window once per painted frame.
    ///
    /// Windows keeps the frameless `WS_POPUP` style and the `WS_EX_NOACTIVATE`
    /// bit in place here; macOS re-asserts its non-activating panel mask.
    /// Returns true when the frame styles had to be rebuilt, which is what the
    /// smoke test counts as a flash risk.
    fn present_window(&self, _window: &winit::window::Window) -> bool {
        false
    }

    /// Called when the pet window was resized so a shell can defer its
    /// post-resize frame fixup.
    fn notify_window_resize(&self) {}

    /// Move and resize in one operation, so the pet anchor cannot jump.
    fn set_window_geometry(
        &self,
        window: &winit::window::Window,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) {
        set_winit_geometry(window, x, y, width, height);
    }

    /// Apply the undecorated, non-activating popup style to our own top-level
    /// windows with this title (bubble, egui context menu). Returns how many
    /// windows were styled.
    fn set_no_activate_for_title(&self, _title: &str) -> usize {
        0
    }

    /// Extra native activation for the settings window.
    ///
    /// egui already sent `ViewportCommand::Focus`. macOS additionally has to
    /// activate the app because the pet itself is a non-activating panel.
    /// Returning true means the focus request has been satisfied.
    fn confirm_settings_focus(&self, _title: &str) -> bool {
        true
    }

    /// Show a tray-icon menu next to the cursor over one of our own views.
    /// AppKit places it itself; the default leaves that to the caller.
    fn show_context_menu_for_window(
        &self,
        _window: &winit::window::Window,
        _menu: &tray_icon::menu::Menu,
    ) {
    }

    /// Install the platform's global input hook. Returns true when the host
    /// wakes the UI on real input instead of polling for it.
    fn install_event_waker(&self, _ctx: &egui::Context) -> bool {
        false
    }

    /// Does the host wake the UI on mouse input?
    fn event_driven_mouse(&self) -> bool {
        false
    }

    /// True when sampling the global pointer is expensive enough that the UI
    /// should only do it when there is evidence of pointer activity.
    fn throttle_pointer_sampling(&self) -> bool {
        false
    }

    /// Sample the global pointer and mouse buttons.
    fn pointer_snapshot(&self) -> PointerSnapshot {
        PointerSnapshot::default()
    }

    /// Is Escape held? Native menus never take focus, so the key is polled.
    fn escape_pressed(&self) -> bool {
        false
    }

    /// Start the shell's native context menu, if it has one.
    fn create_menu(&self, _ctx: &egui::Context) -> Option<Box<dyn PlatformMenu>> {
        None
    }

    /// Does this platform show the tray menu through the system (`tray-icon`)?
    fn uses_native_tray_menu(&self) -> bool {
        false
    }

    /// Does this platform offer native file chooser dialogs?
    fn supports_native_file_dialogs(&self) -> bool {
        false
    }

    fn choose_pet_import_path(&self) -> Option<PathBuf> {
        None
    }

    fn choose_pet_export_path(&self, _default_dir: &Path, _default_name: &str) -> Option<PathBuf> {
        None
    }

    /// Reveal a folder in the platform file manager.
    fn open_in_file_manager(&self, _path: &Path) -> bool {
        false
    }

    /// Number of fallback global pointer polls since startup.
    #[cfg(feature = "test-hooks")]
    fn cursor_poll_count(&self) -> u64 {
        0
    }

    #[cfg(feature = "test-hooks")]
    fn mouse_event_count(&self) -> u64 {
        0
    }

    #[cfg(feature = "test-hooks")]
    fn mouse_position_valid(&self) -> bool {
        false
    }

    #[cfg(feature = "test-hooks")]
    fn is_window_key_for_title(&self, _title: &str) -> bool {
        false
    }
}

/// Portable geometry: winit places and resizes in two steps, which is fine on
/// backends that never paint an intermediate frame.
pub fn set_winit_geometry(window: &winit::window::Window, x: f64, y: f64, width: f64, height: f64) {
    window.set_outer_position(winit::dpi::LogicalPosition::new(x, y));
    let _ = window.request_inner_size(winit::dpi::LogicalSize::new(width, height));
}

/// Host without a native backend: the app falls back to egui and winit
/// behaviour. Used by this crate's own tests and by headless checks.
#[derive(Debug, Default)]
pub struct PortableHost;

impl PlatformHost for PortableHost {}
