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

/// A rectangle in **physical pixels**, using the same origin as Win32
/// (`GetWindowRect`, monitor bounds, cursor coordinates).
///
/// Everything that has to line up across monitors with different DPI factors
/// (window restore, work-area clamping, gravity's floor) goes through this
/// type; logical `egui` points only exist at the winit boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl PhysicalRect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn right(&self) -> i32 {
        self.x.saturating_add(self.width)
    }

    pub const fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height)
    }

    pub const fn center(&self) -> (i32, i32) {
        (
            self.x.saturating_add(self.width / 2),
            self.y.saturating_add(self.height / 2),
        )
    }

    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
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

    /// Preferred CJK font files, in priority order. Shells provide the system
    /// paths; the shared UI only reads and installs the first usable file.
    fn cjk_font_candidates(&self) -> Vec<PathBuf> {
        Vec::new()
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

    /// Move and resize in physical pixels.
    ///
    /// Position memory, gravity and multi-monitor clamping all reason in
    /// physical pixels; only the portable fallback converts back to logical
    /// points for winit.
    fn set_window_geometry_physical(&self, window: &winit::window::Window, rect: PhysicalRect) {
        // winit takes physical positions and sizes directly, so the portable
        // path does not have to guess a scale factor (which would be wrong
        // when the target monitor has a different DPI than the current one).
        // The pet window is undecorated, so inner size == outer size.
        window.set_outer_position(winit::dpi::PhysicalPosition::new(rect.x, rect.y));
        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
            rect.width.max(1) as u32,
            rect.height.max(1) as u32,
        ));
    }

    /// Work area (the monitor minus taskbar/dock) of the monitor nearest to a
    /// physical screen point, in physical pixels.
    ///
    /// `None` makes the shared UI fall back to winit's full monitor bounds.
    fn monitor_work_area(&self, _x: i32, _y: i32) -> Option<PhysicalRect> {
        None
    }

    /// Can the shell register/unregister a login item?
    fn autostart_supported(&self) -> bool {
        false
    }

    /// Is the login item currently registered? This reads the real OS state,
    /// not a config copy, so it stays correct if the user edits it elsewhere.
    fn autostart_enabled(&self) -> bool {
        false
    }

    /// Register or remove the login item. Errors are shown in the settings UI.
    fn set_autostart(&self, _enabled: bool) -> Result<(), String> {
        Err("当前平台不支持在设置里配置开机自启动".to_string())
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

    /// Prepare an activatable popup (the conversation window) before it is
    /// shown: remove native decorations, keep it activatable, and disable the
    /// OS show/hide transition. Hosts without native window styling simply
    /// report success. The app retries until this returns true.
    fn prepare_activatable_popup_window(&self, _title: &str) -> bool {
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

    /// Did the shell switch off the OS show/hide transition for its popup
    /// windows (bubble, context menu)? Windows otherwise fades — and with the
    /// "fade or slide menus" accessibility option, slides — them in, which
    /// fights the bubble's own entry animation.
    #[cfg(feature = "test-hooks")]
    fn popup_transitions_disabled(&self) -> bool {
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
