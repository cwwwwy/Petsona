#[derive(Debug, Clone, Copy, Default)]
pub struct PointerSnapshot {
    pub position: Option<(f64, f64)>,
    pub primary_down: Option<bool>,
    pub secondary_down: Option<bool>,
}

#[cfg(target_os = "windows")]
mod no_activate_proc {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, DefWindowProcW, SetWindowLongPtrW, GWLP_WNDPROC, GWL_EXSTYLE, GWL_STYLE,
        MA_NOACTIVATE, STYLESTRUCT, WM_MOUSEACTIVATE, WM_NCDESTROY, WM_STYLECHANGING, WNDPROC,
        WS_BORDER, WS_CAPTION, WS_DLGFRAME, WS_EX_NOACTIVATE, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
        WS_POPUP, WS_SYSMENU,
    };

    thread_local! {
        static PREVIOUS: RefCell<HashMap<isize, isize>> = RefCell::new(HashMap::new());
    }

    pub fn install(hwnd: HWND) {
        let key = hwnd as isize;
        if PREVIOUS.with(|previous| previous.borrow().contains_key(&key)) {
            return;
        }
        let previous = unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWLP_WNDPROC,
                no_activate_wndproc as *const () as isize,
            )
        };
        if previous != 0 {
            PREVIOUS.with(|previous_procs| {
                previous_procs.borrow_mut().insert(key, previous);
            });
        }
    }

    unsafe extern "system" fn no_activate_wndproc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_MOUSEACTIVATE {
            return MA_NOACTIVATE as LRESULT;
        }
        if message == WM_STYLECHANGING && lparam != 0 {
            let styles = unsafe { &mut *(lparam as *mut STYLESTRUCT) };
            let index = wparam as i32;
            if index == GWL_STYLE {
                let frame = WS_CAPTION
                    | WS_BORDER
                    | WS_DLGFRAME
                    | WS_SYSMENU
                    | WS_MINIMIZEBOX
                    | WS_MAXIMIZEBOX;
                styles.styleNew = (styles.styleNew & !frame) | WS_POPUP;
            } else if index == GWL_EXSTYLE {
                styles.styleNew |= WS_EX_NOACTIVATE;
            }
        }

        let key = hwnd as isize;
        let previous = PREVIOUS.with(|previous_procs| {
            previous_procs
                .borrow()
                .get(&key)
                .copied()
                .unwrap_or_default()
        });
        let result = if previous == 0 {
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        } else {
            let previous: WNDPROC = unsafe { std::mem::transmute(previous) };
            unsafe { CallWindowProcW(previous, hwnd, message, wparam, lparam) }
        };

        if message == WM_NCDESTROY {
            PREVIOUS.with(|previous_procs| {
                previous_procs.borrow_mut().remove(&key);
            });
        }
        result
    }
}

#[cfg(feature = "test-hooks")]
static CURSOR_POLL_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(feature = "test-hooks")]
fn note_cursor_poll() {
    CURSOR_POLL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(feature = "test-hooks")]
pub fn cursor_poll_count() -> u64 {
    CURSOR_POLL_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(target_os = "windows")]
static MOUSE_HOOK: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
#[cfg(target_os = "windows")]
static MOUSE_CONTEXT: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();
#[cfg(target_os = "windows")]
static MOUSE_X: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
#[cfg(target_os = "windows")]
static MOUSE_Y: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
#[cfg(target_os = "windows")]
static MOUSE_POSITION_VALID: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "windows")]
pub fn install_mouse_waker(ctx: &egui::Context) -> bool {
    use std::sync::atomic::Ordering;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, WH_MOUSE_LL};

    if MOUSE_HOOK.load(Ordering::Relaxed) != 0 {
        return true;
    }
    let _ = MOUSE_CONTEXT.set(ctx.clone());
    {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

        let mut point = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut point) } != 0 {
            MOUSE_X.store(point.x, Ordering::Relaxed);
            MOUSE_Y.store(point.y, Ordering::Relaxed);
            MOUSE_POSITION_VALID.store(true, Ordering::Relaxed);
        }
    }
    let hook =
        unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), std::ptr::null_mut(), 0) };
    if hook.is_null() {
        return false;
    }
    MOUSE_HOOK.store(hook as isize, Ordering::Relaxed);
    true
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn mouse_hook_proc(
    code: i32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    use std::sync::atomic::Ordering;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, MSLLHOOKSTRUCT, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
        WM_RBUTTONDOWN, WM_RBUTTONUP,
    };

    if code >= 0 {
        let hook_data = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
        MOUSE_X.store(hook_data.pt.x, Ordering::Relaxed);
        MOUSE_Y.store(hook_data.pt.y, Ordering::Relaxed);
        MOUSE_POSITION_VALID.store(true, Ordering::Relaxed);
        let message = wparam as u32;
        if matches!(
            message,
            WM_MOUSEMOVE
                | WM_LBUTTONDOWN
                | WM_LBUTTONUP
                | WM_RBUTTONDOWN
                | WM_RBUTTONUP
                | WM_MOUSEWHEEL
        ) {
            if let Some(ctx) = MOUSE_CONTEXT.get() {
                ctx.request_repaint();
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

#[cfg(target_os = "windows")]
pub fn event_driven_mouse() -> bool {
    MOUSE_HOOK.load(std::sync::atomic::Ordering::Relaxed) != 0
}

#[cfg(all(target_os = "windows", feature = "test-hooks"))]
pub fn mouse_position_valid() -> bool {
    MOUSE_POSITION_VALID.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(all(not(target_os = "windows"), feature = "test-hooks"))]
pub fn mouse_position_valid() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn event_driven_mouse() -> bool {
    false
}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::{Path, PathBuf};

    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSApplication, NSEvent, NSModalResponseOK, NSOpenPanel, NSSavePanel, NSScreen, NSView,
        NSWindowStyleMask,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, NSURL};
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

    pub fn choose_pet_import_path() -> Option<PathBuf> {
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

    pub fn choose_pet_export_path(default_dir: &Path, default_name: &str) -> Option<PathBuf> {
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

    fn appkit_window(
        window: &winit::window::Window,
    ) -> Option<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
        let handle = window.window_handle().ok()?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return None;
        };
        // winit owns the view for the lifetime of the WindowHandle. The
        // handle is only borrowed for this synchronous AppKit call.
        let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        view.window()
    }

    pub fn set_no_activate(window: &winit::window::Window) {
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

    pub fn set_no_activate_for_title(title: &str) -> usize {
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
    pub fn focus_window_for_title(title: &str) -> usize {
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
    pub fn is_window_key_for_title(title: &str) -> bool {
        let Some(main_thread) = MainThreadMarker::new() else {
            return false;
        };
        let app = NSApplication::sharedApplication(main_thread);
        app.windows()
            .iter()
            .any(|window| window.title().to_string() == title && window.isKeyWindow())
    }

    pub fn escape_pressed() -> bool {
        // Menus deliberately do not become key windows, so AppKit will not
        // reliably deliver Escape to egui. Poll the HID event source instead.
        unsafe { CGEventSourceKeyState(HID_SYSTEM_STATE, ESCAPE_KEY_CODE) }
    }

    #[allow(dead_code)]
    pub fn primary_button_down() -> Option<bool> {
        let buttons = NSEvent::pressedMouseButtons();
        Some(buttons & 1 != 0)
    }

    #[allow(dead_code)]
    pub fn secondary_button_down() -> Option<bool> {
        let buttons = NSEvent::pressedMouseButtons();
        Some(buttons & 2 != 0)
    }

    pub fn pointer_snapshot() -> super::PointerSnapshot {
        let position = global_cursor_position();
        let buttons = NSEvent::pressedMouseButtons();
        super::PointerSnapshot {
            position,
            primary_down: Some(buttons & 1 != 0),
            secondary_down: Some(buttons & 2 != 0),
        }
    }

    /// Apply the pet's position and size as one AppKit frame change. Calling
    /// winit's `set_outer_position` and `request_inner_size` separately lets
    /// macOS paint an intermediate frame while the scale slider is moving.
    pub fn set_window_geometry(
        window: &winit::window::Window,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) {
        let Some(ns_window) = super::macos::appkit_window(window) else {
            window.set_outer_position(winit::dpi::LogicalPosition::new(x, y));
            let _ = window.request_inner_size(winit::dpi::LogicalSize::new(width, height));
            return;
        };
        let main_screen_height = unsafe { CGDisplayBounds(CGMainDisplayID()).size.height };
        let frame = appkit_frame_for_winit(x, y, width, height, main_screen_height);
        ns_window.setFrame_display_animate(frame, true, false);
    }

    pub fn global_cursor_position() -> Option<(f64, f64)> {
        #[cfg(feature = "test-hooks")]
        super::note_cursor_poll();
        let point = NSEvent::mouseLocation();
        let main_thread = MainThreadMarker::new()?;
        let main_screen = NSScreen::mainScreen(main_thread)?;
        let main_height = unsafe { CGDisplayBounds(CGMainDisplayID()).size.height };

        // NSEvent reports AppKit points with the origin at the bottom-left.
        // winit exposes physical screen coordinates with the origin at the
        // top-left, so flip Y and apply the scale of the display containing
        // the cursor. For a cursor near the pet this is also the pet window's
        // scale, including a mixed-Retina setup.
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
}

/// Re-establish per-pixel transparency.
///
/// winit creates transparent windows by giving DWM an empty blur region, and
/// that is per-window state which a forced frame recompute can drop, so it is
/// re-applied after changing the window styles.
#[cfg(target_os = "windows")]
pub fn enable_transparency(window: &winit::window::Window) {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DwmEnableBlurBehindWindow, DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND,
    };
    use windows_sys::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject};

    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = handle.hwnd.get() as *mut core::ffi::c_void;
    unsafe {
        let region = CreateRectRgn(0, 0, -1, -1);
        let blur = DWM_BLURBEHIND {
            dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
            fEnable: 1,
            hRgnBlur: region,
            fTransitionOnMaximized: 0,
        };
        DwmEnableBlurBehindWindow(hwnd, &blur);
        DeleteObject(region);
    }
}

/// Remove every classic frame style from a window.
///
/// `decorations(false)` only hides the caption; the window still carries
/// `WS_CAPTION | WS_BORDER | WS_DLGFRAME | WS_SYSMENU | WS_MINIMIZEBOX |
/// WS_MAXIMIZEBOX`. Windows then redraws that non-client frame whenever the
/// window state changes (activation, a style change, a resize), which is the
/// border that flashed around the pet. A pure `WS_POPUP` window has no frame
/// to draw at all.
#[cfg(target_os = "windows")]
pub fn strip_frame_styles(window: &winit::window::Window) -> bool {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_BORDER, WS_CAPTION, WS_DLGFRAME,
        WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU,
    };

    let Ok(handle) = window.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return false;
    };
    let hwnd = handle.hwnd.get() as *mut core::ffi::c_void;
    let frame = WS_CAPTION | WS_BORDER | WS_DLGFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let wanted = (style & !frame) | WS_POPUP;
        if style == wanted {
            return false;
        }
        tracing::info!(
            style = format_args!("0x{style:08X}"),
            wanted = format_args!("0x{wanted:08X}"),
            "restoring frameless window style"
        );
        SetWindowLongPtrW(hwnd, GWL_STYLE, wanted as isize);
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
        true
    }
}

/// Keep a window from ever becoming the active window.
///
/// Activating the pet window made Windows repaint its frame state (the flash
/// seen on the first drag) and stole focus from whatever the user was doing.
#[cfg(target_os = "windows")]
pub fn set_no_activate(window: &winit::window::Window) {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE,
    };

    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = handle.hwnd.get() as *mut core::ffi::c_void;
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if style & WS_EX_NOACTIVATE == 0 {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (style | WS_EX_NOACTIVATE) as isize);
        }
        no_activate_proc::install(hwnd);
    }
}

#[cfg(target_os = "macos")]
pub fn set_no_activate(window: &winit::window::Window) {
    macos::set_no_activate(window);
}

/// Same as [`set_no_activate`] for a window identified by its title, used for
/// the menus that are created on demand. Returns how many windows were changed.
#[cfg(target_os = "windows")]
pub fn set_no_activate_for_title(title: &str) -> usize {
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, TRUE};
    use windows_sys::Win32::Graphics::Dwm::{
        DwmEnableBlurBehindWindow, DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND,
    };
    use windows_sys::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowLongPtrW, GetWindowTextW, GetWindowThreadProcessId,
        SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_BORDER, WS_CAPTION, WS_DLGFRAME, WS_EX_NOACTIVATE,
        WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU,
    };

    struct Lookup<'a> {
        title: &'a str,
        hits: usize,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let lookup = unsafe { &mut *(lparam as *mut Lookup<'_>) };
        let mut buffer = [0u16; 256];
        let len = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
        if len <= 0 {
            return TRUE;
        }
        let text = String::from_utf16_lossy(&buffer[..len as usize]);
        if text != lookup.title {
            return TRUE;
        }
        let mut owner = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut owner);
        }
        if owner != std::process::id() {
            return TRUE;
        }
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            if style & WS_EX_NOACTIVATE == 0 {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (style | WS_EX_NOACTIVATE) as isize);
            }
            // The popup windows are undecorated too, so give them the same
            // frame-free style as the pet.
            let frame =
                WS_CAPTION | WS_BORDER | WS_DLGFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
            let window_style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            let wanted = (window_style & !frame) | WS_POPUP;
            if window_style != wanted {
                SetWindowLongPtrW(hwnd, GWL_STYLE, wanted as isize);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
                // Keep the popup transparent after the frame recompute.
                let region = CreateRectRgn(0, 0, -1, -1);
                let blur = DWM_BLURBEHIND {
                    dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
                    fEnable: 1,
                    hRgnBlur: region,
                    fTransitionOnMaximized: 0,
                };
                DwmEnableBlurBehindWindow(hwnd, &blur);
                DeleteObject(region);
            }
            no_activate_proc::install(hwnd);
        }
        lookup.hits += 1;
        TRUE
    }

    let mut lookup = Lookup { title, hits: 0 };
    unsafe {
        EnumWindows(Some(visit), &mut lookup as *mut _ as LPARAM);
    }
    lookup.hits
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn set_no_activate_for_title(_title: &str) -> usize {
    0
}

#[cfg(target_os = "macos")]
pub fn set_no_activate_for_title(title: &str) -> usize {
    macos::set_no_activate_for_title(title)
}

#[cfg(target_os = "macos")]
pub fn focus_window_for_title(title: &str) -> usize {
    macos::focus_window_for_title(title)
}

#[cfg(all(target_os = "macos", feature = "test-hooks"))]
pub fn is_window_key_for_title(title: &str) -> bool {
    macos::is_window_key_for_title(title)
}

/// Is Escape held? Menus do not take focus, so the key has to be polled.
#[cfg(target_os = "windows")]
pub fn escape_pressed() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};

    let state = unsafe { GetAsyncKeyState(VK_ESCAPE as i32) };
    (state as u16 & 0x8000) != 0
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn escape_pressed() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub fn escape_pressed() -> bool {
    macos::escape_pressed()
}

/// Is the left mouse button held right now?
///
/// Dragging the pet is driven by the application instead of the OS modal move
/// loop, and that loop swallows the button-release event, so the state has to
/// come from the system. `None` means "not available on this platform".
#[cfg(target_os = "windows")]
pub fn primary_button_down() -> Option<bool> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};

    let state = unsafe { GetAsyncKeyState(VK_LBUTTON as i32) };
    Some((state as u16 & 0x8000) != 0)
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn primary_button_down() -> Option<bool> {
    None
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn primary_button_down() -> Option<bool> {
    macos::primary_button_down()
}

/// Is the right mouse button held right now?
#[cfg(target_os = "windows")]
pub fn secondary_button_down() -> Option<bool> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_RBUTTON};

    let state = unsafe { GetAsyncKeyState(VK_RBUTTON as i32) };
    Some((state as u16 & 0x8000) != 0)
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn secondary_button_down() -> Option<bool> {
    None
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn secondary_button_down() -> Option<bool> {
    macos::secondary_button_down()
}

#[cfg(target_os = "windows")]
pub fn pointer_snapshot() -> PointerSnapshot {
    PointerSnapshot {
        position: global_cursor_position(),
        primary_down: primary_button_down(),
        secondary_down: secondary_button_down(),
    }
}

#[cfg(target_os = "macos")]
pub fn pointer_snapshot() -> PointerSnapshot {
    macos::pointer_snapshot()
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn pointer_snapshot() -> PointerSnapshot {
    PointerSnapshot::default()
}

#[cfg(target_os = "macos")]
pub fn choose_pet_import_path() -> Option<std::path::PathBuf> {
    macos::choose_pet_import_path()
}

#[cfg(target_os = "macos")]
pub fn choose_pet_export_path(
    default_dir: &std::path::Path,
    default_name: &str,
) -> Option<std::path::PathBuf> {
    macos::choose_pet_export_path(default_dir, default_name)
}

/// Reveal a folder in the platform file manager.
///
/// Used by "打开宠物库目录" so the user can drop pet packages in by hand.
pub fn open_in_file_manager(path: &std::path::Path) -> bool {
    #[cfg(target_os = "windows")]
    let (program, args) = ("explorer.exe", vec![path.as_os_str().to_os_string()]);
    #[cfg(target_os = "macos")]
    let (program, args) = ("open", vec![path.as_os_str().to_os_string()]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let (program, args) = ("xdg-open", vec![path.as_os_str().to_os_string()]);

    std::process::Command::new(program)
        .args(args)
        .spawn()
        .is_ok()
}

/// Set position and size in one operation so the pet anchor cannot jump.
#[cfg(target_os = "windows")]
pub fn set_window_geometry(
    window: &winit::window::Window,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER,
    };

    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let scale = window.scale_factor().max(0.1);
    let hwnd = handle.hwnd.get() as *mut core::ffi::c_void;
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            (x * scale).round() as i32,
            (y * scale).round() as i32,
            (width * scale).round() as i32,
            (height * scale).round() as i32,
            SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE,
        );
    }
}

#[cfg(target_os = "macos")]
pub fn set_window_geometry(
    window: &winit::window::Window,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    macos::set_window_geometry(window, x, y, width, height);
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn set_window_geometry(
    window: &winit::window::Window,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    window.set_outer_position(winit::dpi::LogicalPosition::new(x, y));
    let _ = window.request_inner_size(winit::dpi::LogicalSize::new(width, height));
}

/// Best-effort global cursor position.
///
/// Windows is the important case for pixel-level click-through: once the
/// window has `WS_EX_TRANSPARENT`, egui may stop receiving cursor updates, so
/// we ask Win32 directly and can restore interaction when the cursor moves
/// back over an opaque pixel.
#[cfg(target_os = "windows")]
pub fn global_cursor_position() -> Option<(f64, f64)> {
    if event_driven_mouse() && MOUSE_POSITION_VALID.load(std::sync::atomic::Ordering::Relaxed) {
        return Some((
            MOUSE_X.load(std::sync::atomic::Ordering::Relaxed) as f64,
            MOUSE_Y.load(std::sync::atomic::Ordering::Relaxed) as f64,
        ));
    }
    #[cfg(feature = "test-hooks")]
    note_cursor_poll();
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut point = POINT { x: 0, y: 0 };
    let ok = unsafe { GetCursorPos(&mut point) };
    (ok != 0).then_some((point.x as f64, point.y as f64))
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn global_cursor_position() -> Option<(f64, f64)> {
    #[cfg(feature = "test-hooks")]
    note_cursor_poll();
    None
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn global_cursor_position() -> Option<(f64, f64)> {
    macos::global_cursor_position()
}

#[cfg(target_os = "windows")]
pub fn clear_dwm_frame(window: &winit::window::Window) {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMWA_NCRENDERING_POLICY,
    };

    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = handle.hwnd.get() as *mut core::ffi::c_void;
    let policy = DWMNCRP_DISABLED;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY as u32,
            &policy as *const _ as *const _,
            std::mem::size_of_val(&policy) as u32,
        );
    }
}
