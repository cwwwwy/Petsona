//! AppKit / NSEvent / CoreGraphics backend for the Petsona macOS shell.
//!
//! This is the code that used to sit behind `#[cfg(target_os = "macos")]` in
//! `petsona-app`; the conversion to logical winit coordinates and the AppKit
//! frame flipping are unchanged.

use std::path::{Path, PathBuf};

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSEvent, NSModalResponseOK, NSOpenPanel, NSSavePanel, NSScreen, NSView,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, NSURL};
use petsona_app::platform::{PlatformHost, PointerSnapshot};
use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> CoreGraphicsRect;
}

#[repr(C)]
struct CoreGraphicsPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
struct CoreGraphicsSize {
    width: f64,
    height: f64,
}

#[repr(C)]
struct CoreGraphicsRect {
    origin: CoreGraphicsPoint,
    size: CoreGraphicsSize,
}

const HID_SYSTEM_STATE: i32 = 1;
const ESCAPE_KEY_CODE: u16 = 53;

/// `PlatformHost` backed by AppKit, NSEvent and CoreGraphics.
#[derive(Debug, Default)]
pub struct MacHost;

impl MacHost {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformHost for MacHost {
    fn name(&self) -> &'static str {
        "macos"
    }

    fn cjk_font_candidates(&self) -> Vec<PathBuf> {
        [
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect()
    }

    fn present_window(&self, window: &winit::window::Window) -> bool {
        // AppKit needs the non-activating panel mask; winit's
        // `with_active(false)` only affects initial creation.
        apply_nonactivating_panel(window);
        false
    }

    fn set_no_activate_for_title(&self, title: &str) -> usize {
        style_windows_with_title(title)
    }

    fn confirm_settings_focus(&self, title: &str) -> bool {
        activate_windows_with_title(title) > 0
    }

    fn show_context_menu_for_window(
        &self,
        window: &winit::window::Window,
        menu: &tray_icon::menu::Menu,
    ) {
        use tray_icon::menu::ContextMenu as _;

        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };

        // Passing None asks AppKit to use the current mouse location in screen
        // coordinates, so it handles menu-bar offsets, Retina scaling and
        // multi-monitor placement itself.
        unsafe {
            let _ = menu.show_context_menu_for_nsview(handle.ns_view.as_ptr(), None);
        }
    }

    fn throttle_pointer_sampling(&self) -> bool {
        true
    }

    fn pointer_snapshot(&self) -> PointerSnapshot {
        let position = global_cursor_position();
        let buttons = NSEvent::pressedMouseButtons();
        PointerSnapshot {
            position,
            primary_down: Some(buttons & 1 != 0),
            secondary_down: Some(buttons & 2 != 0),
        }
    }

    fn escape_pressed(&self) -> bool {
        // Menus deliberately do not become key windows, so AppKit will not
        // reliably deliver Escape to egui. Poll the HID event source instead.
        unsafe { CGEventSourceKeyState(HID_SYSTEM_STATE, ESCAPE_KEY_CODE) }
    }

    fn set_window_geometry(
        &self,
        window: &winit::window::Window,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) {
        set_pet_geometry(window, x, y, width, height);
    }

    fn uses_native_tray_menu(&self) -> bool {
        true
    }

    fn supports_native_file_dialogs(&self) -> bool {
        true
    }

    fn choose_pet_import_path(&self) -> Option<PathBuf> {
        choose_import_path()
    }

    fn choose_pet_export_path(&self, default_dir: &Path, default_name: &str) -> Option<PathBuf> {
        choose_export_path(default_dir, default_name)
    }

    fn open_in_file_manager(&self, path: &Path) -> bool {
        std::process::Command::new("open")
            .arg(path.as_os_str())
            .spawn()
            .is_ok()
    }

    #[cfg(feature = "test-hooks")]
    fn cursor_poll_count(&self) -> u64 {
        CURSOR_POLL_COUNT.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[cfg(feature = "test-hooks")]
    fn is_window_key_for_title(&self, title: &str) -> bool {
        window_is_key(title)
    }
}

#[cfg(feature = "test-hooks")]
static CURSOR_POLL_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(feature = "test-hooks")]
fn note_cursor_poll() {
    CURSOR_POLL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

fn appkit_to_winit_cursor(
    point_x: f64,
    point_y: f64,
    main_screen_height: f64,
    screen_scale: f64,
) -> (f64, f64) {
    (
        point_x * screen_scale,
        (main_screen_height - point_y) * screen_scale,
    )
}

fn appkit_frame_for_winit(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    main_screen_height: f64,
) -> NSRect {
    NSRect::new(
        NSPoint::new(x, main_screen_height - height - y),
        NSSize::new(width, height),
    )
}

fn path_from_url(url: Option<objc2::rc::Retained<NSURL>>) -> Option<PathBuf> {
    url.and_then(|url| url.path())
        .map(|path| PathBuf::from(path.to_string()))
}

fn file_url(path: &Path) -> objc2::rc::Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

fn choose_import_path() -> Option<PathBuf> {
    let main_thread = MainThreadMarker::new()?;
    let panel = NSOpenPanel::openPanel(main_thread);
    let title = NSString::from_str("导入宠物");
    let prompt = NSString::from_str("选择");
    panel.setTitle(Some(&title));
    panel.setPrompt(Some(&prompt));
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(true);
    panel.setAllowsMultipleSelection(false);
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    path_from_url(panel.URL())
}

fn choose_export_path(default_dir: &Path, default_name: &str) -> Option<PathBuf> {
    let main_thread = MainThreadMarker::new()?;
    let panel = NSSavePanel::savePanel(main_thread);
    let title = NSString::from_str("导出宠物");
    let prompt = NSString::from_str("保存");
    let name = NSString::from_str(default_name);
    panel.setTitle(Some(&title));
    panel.setPrompt(Some(&prompt));
    panel.setNameFieldStringValue(&name);
    panel.setCanCreateDirectories(true);
    panel.setAllowsOtherFileTypes(true);
    let directory = file_url(default_dir);
    panel.setDirectoryURL(Some(&directory));
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    let path = path_from_url(panel.URL())?;
    if path.extension().is_some() {
        Some(path)
    } else {
        Some(path.with_extension("zip"))
    }
}

fn appkit_window(window: &winit::window::Window) -> Option<objc2::rc::Retained<NSWindow>> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    // winit owns the view for the lifetime of the WindowHandle. The handle is
    // only borrowed for this synchronous AppKit call.
    let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    view.window()
}

fn apply_nonactivating_panel(window: &winit::window::Window) {
    let Some(_main_thread) = MainThreadMarker::new() else {
        return;
    };
    let Some(ns_window) = appkit_window(window) else {
        return;
    };
    let style = ns_window.styleMask();
    let wanted = style | NSWindowStyleMask::NonactivatingPanel;
    if wanted != style {
        ns_window.setStyleMask(wanted);
    }
}

fn style_windows_with_title(title: &str) -> usize {
    let Some(main_thread) = MainThreadMarker::new() else {
        return 0;
    };
    let app = NSApplication::sharedApplication(main_thread);
    let windows = app.windows();
    let mut hits = 0;
    for window in windows.iter() {
        if window.title().to_string() != title {
            continue;
        }
        let style = window.styleMask();
        let wanted = style | NSWindowStyleMask::NonactivatingPanel;
        if wanted != style {
            window.setStyleMask(wanted);
        }
        hits += 1;
    }
    hits
}

/// Activate a regular child window after it is opened from the
/// non-activating pet/menu. The pet deliberately uses
/// `NSWindowStyleMask::NonactivatingPanel`, but that mask must never be
/// applied to the settings window because it prevents text fields from
/// becoming the key window.
fn activate_windows_with_title(title: &str) -> usize {
    let Some(main_thread) = MainThreadMarker::new() else {
        return 0;
    };
    let app = NSApplication::sharedApplication(main_thread);
    let windows = app.windows();
    let matching_windows = windows
        .iter()
        .filter(|window| window.title().to_string() == title)
        .count();
    if matching_windows == 0 {
        return 0;
    }
    app.activate();
    let mut hits = 0;
    for window in windows.iter() {
        if window.title().to_string() != title {
            continue;
        }
        window.makeKeyAndOrderFront(None);
        hits += 1;
    }
    hits
}

#[cfg(feature = "test-hooks")]
fn window_is_key(title: &str) -> bool {
    let Some(main_thread) = MainThreadMarker::new() else {
        return false;
    };
    let app = NSApplication::sharedApplication(main_thread);
    app.windows()
        .iter()
        .any(|window| window.title().to_string() == title && window.isKeyWindow())
}

/// Apply the pet's position and size as one AppKit frame change. Calling
/// winit's `set_outer_position` and `request_inner_size` separately lets macOS
/// paint an intermediate frame while the scale slider is moving.
fn set_pet_geometry(window: &winit::window::Window, x: f64, y: f64, width: f64, height: f64) {
    let Some(ns_window) = appkit_window(window) else {
        petsona_app::platform::set_winit_geometry(window, x, y, width, height);
        return;
    };
    let main_screen_height = unsafe { CGDisplayBounds(CGMainDisplayID()).size.height };
    let frame = appkit_frame_for_winit(x, y, width, height, main_screen_height);
    ns_window.setFrame_display_animate(frame, true, false);
}

fn global_cursor_position() -> Option<(f64, f64)> {
    #[cfg(feature = "test-hooks")]
    note_cursor_poll();
    let point = NSEvent::mouseLocation();
    let main_thread = MainThreadMarker::new()?;
    let main_screen = NSScreen::mainScreen(main_thread)?;
    let main_height = unsafe { CGDisplayBounds(CGMainDisplayID()).size.height };

    // NSEvent reports AppKit points with the origin at the bottom-left. winit
    // exposes physical screen coordinates with the origin at the top-left, so
    // flip Y and apply the scale of the display containing the cursor. For a
    // cursor near the pet this is also the pet window's scale, including a
    // mixed-Retina setup.
    let screen_scale = NSScreen::screens(main_thread)
        .iter()
        .find_map(|screen| {
            let frame = screen.frame();
            let x = point.x;
            let y = point.y;
            let inside = x >= frame.origin.x
                && x <= frame.origin.x + frame.size.width
                && y >= frame.origin.y
                && y <= frame.origin.y + frame.size.height;
            inside.then(|| screen.backingScaleFactor())
        })
        .unwrap_or_else(|| main_screen.backingScaleFactor());

    Some(appkit_to_winit_cursor(
        point.x,
        point.y,
        main_height,
        screen_scale,
    ))
}

#[cfg(test)]
mod tests {
    use super::{appkit_frame_for_winit, appkit_to_winit_cursor};

    #[test]
    fn frame_geometry_flips_winit_top_left_to_appkit_bottom_left() {
        let frame = appkit_frame_for_winit(40.0, 100.0, 220.0, 208.0, 900.0);
        assert_eq!(frame.origin.x, 40.0);
        assert_eq!(frame.origin.y, 592.0);
        assert_eq!(frame.size.width, 220.0);
        assert_eq!(frame.size.height, 208.0);
    }

    #[test]
    fn cursor_conversion_uses_the_core_graphics_main_display_height() {
        assert_eq!(
            appkit_to_winit_cursor(40.0, 100.0, 900.0, 2.0),
            (80.0, 1600.0)
        );
    }

    #[test]
    fn cursor_conversion_preserves_negative_secondary_display_coordinates() {
        assert_eq!(
            appkit_to_winit_cursor(-200.0, -120.0, 900.0, 1.0),
            (-200.0, 1020.0)
        );
    }
}
