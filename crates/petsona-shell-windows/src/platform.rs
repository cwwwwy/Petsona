//! Win32 platform backend for the Petsona Windows shell.
//!
//! Everything here used to live behind `#[cfg(target_os = "windows")]` inside
//! `petsona-app`. The notes below each block are kept because they record why
//! the current implementation looks the way it does.

use std::path::Path;
#[cfg(feature = "test-hooks")]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use petsona_app::platform::{PhysicalRect, PlatformHost, PlatformMenu, PointerSnapshot};

use crate::menu::WindowsMenu;

/// Frame fixup waits this long after a resize before touching the styles
/// again. winit only needs a rebuild if Windows actually restored the
/// decorated style.
const CHROME_SETTLE: Duration = Duration::from_millis(200);

#[derive(Default)]
struct ChromeState {
    ready: bool,
    resize_at: Option<Instant>,
}

/// `PlatformHost` backed by Win32.
pub struct WindowsHost {
    chrome: Mutex<ChromeState>,
}

impl Default for WindowsHost {
    fn default() -> Self {
        Self {
            chrome: Mutex::new(ChromeState::default()),
        }
    }
}

impl WindowsHost {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PlatformHost for WindowsHost {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn present_window(&self, window: &winit::window::Window) -> bool {
        use winit::platform::windows::{CornerPreference, WindowExtWindows as _};

        let (first_apply, settled) = {
            let mut state = self.chrome.lock().expect("window chrome state");
            if !state.ready {
                (true, false)
            } else if state
                .resize_at
                .is_some_and(|at| at.elapsed() >= CHROME_SETTLE)
            {
                state.resize_at = None;
                (false, true)
            } else {
                (false, false)
            }
        };

        if first_apply {
            // Apply the chrome exactly once. Touching these attributes every
            // frame made Windows repaint the non-client frame, which is the
            // border that flashed.
            window.set_undecorated_shadow(false);
            window.set_border_color(None);
            window.set_corner_preference(CornerPreference::DoNotRound);
            let changed = strip_frame_styles(window);
            if changed {
                clear_dwm_frame(window);
                enable_transparency(window);
            }
            // Never activate: activation repaints the frame state and would
            // also steal focus from the user's editor.
            set_no_activate(window);
            self.chrome.lock().expect("window chrome state").ready = true;
            return changed;
        }

        if settled {
            // winit only needs a frame rebuild if Windows actually restored
            // the decorated style after a resize. Reapplying DWM state
            // unconditionally was the remaining flash risk.
            let changed = strip_frame_styles(window);
            if changed {
                clear_dwm_frame(window);
                enable_transparency(window);
                set_no_activate(window);
            }
            return changed;
        }

        false
    }

    fn notify_window_resize(&self) {
        self.chrome.lock().expect("window chrome state").resize_at = Some(Instant::now());
    }

    fn set_window_geometry(
        &self,
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

    fn set_window_geometry_physical(&self, window: &winit::window::Window, rect: PhysicalRect) {
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
        let hwnd = handle.hwnd.get() as *mut core::ffi::c_void;
        unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                rect.x,
                rect.y,
                rect.width.max(1),
                rect.height.max(1),
                SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE,
            );
        }
    }

    fn monitor_work_area(&self, x: i32, y: i32) -> Option<PhysicalRect> {
        monitor_work_area_physical(x, y)
    }

    fn autostart_supported(&self) -> bool {
        true
    }

    fn autostart_enabled(&self) -> bool {
        autostart::is_enabled()
    }

    fn set_autostart(&self, enabled: bool) -> Result<(), String> {
        if enabled {
            autostart::enable()
        } else {
            autostart::disable()
        }
    }

    fn set_no_activate_for_title(&self, title: &str) -> usize {
        style_popup_window(title)
    }

    fn install_event_waker(&self, ctx: &egui::Context) -> bool {
        install_mouse_waker(ctx)
    }

    fn event_driven_mouse(&self) -> bool {
        MOUSE_HOOK.load(Ordering::Relaxed) != 0
    }

    fn pointer_snapshot(&self) -> PointerSnapshot {
        PointerSnapshot {
            position: global_cursor_position(),
            primary_down: primary_button_down(),
            secondary_down: secondary_button_down(),
        }
    }

    fn escape_pressed(&self) -> bool {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};

        let state = unsafe { GetAsyncKeyState(VK_ESCAPE as i32) };
        (state as u16 & 0x8000) != 0
    }

    fn create_menu(&self, ctx: &egui::Context) -> Option<Box<dyn PlatformMenu>> {
        match WindowsMenu::start(ctx) {
            Ok(menu) => {
                tracing::info!("Win32 native menu thread started");
                Some(Box::new(menu))
            }
            Err(error) => {
                tracing::warn!(%error, "Win32 native menu unavailable; using egui fallback");
                None
            }
        }
    }

    fn open_in_file_manager(&self, path: &Path) -> bool {
        std::process::Command::new("explorer.exe")
            .arg(path.as_os_str())
            .spawn()
            .is_ok()
    }

    #[cfg(feature = "test-hooks")]
    fn cursor_poll_count(&self) -> u64 {
        CURSOR_POLL_COUNT.load(Ordering::Relaxed)
    }

    #[cfg(feature = "test-hooks")]
    fn mouse_event_count(&self) -> u64 {
        MOUSE_EVENT_COUNT.load(Ordering::Relaxed)
    }

    #[cfg(feature = "test-hooks")]
    fn mouse_position_valid(&self) -> bool {
        MOUSE_POSITION_VALID.load(Ordering::Relaxed)
    }

    fn disable_window_animation_for_title(&self, title: &str) -> usize {
        disable_window_animation_by_title(title)
    }

    #[cfg(feature = "test-hooks")]
    fn popup_transitions_disabled(&self) -> bool {
        POPUP_TRANSITIONS_DISABLED.load(Ordering::Relaxed)
    }
}

/// Subclass every Petsona window so it can never be activated.
///
/// The window procedure also repairs `WM_STYLECHANGING`: winit resets
/// `GWL_STYLE`/`GWL_EXSTYLE` on some state changes, which is what made the
/// decorated frame flash back in.
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
static CURSOR_POLL_COUNT: AtomicU64 = AtomicU64::new(0);
/// Set once a popup window (bubble / menu) had its DWM show transition
/// disabled; the smoke test uses it as a regression guard for the bubble entry.
#[cfg(feature = "test-hooks")]
static POPUP_TRANSITIONS_DISABLED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "test-hooks")]
fn note_cursor_poll() {
    CURSOR_POLL_COUNT.fetch_add(1, Ordering::Relaxed);
}

static MOUSE_HOOK: AtomicIsize = AtomicIsize::new(0);
static MOUSE_CONTEXT: OnceLock<egui::Context> = OnceLock::new();
static MOUSE_X: AtomicI32 = AtomicI32::new(0);
static MOUSE_Y: AtomicI32 = AtomicI32::new(0);
static MOUSE_POSITION_VALID: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "test-hooks")]
static MOUSE_EVENT_COUNT: AtomicU64 = AtomicU64::new(0);

/// Install a `WH_MOUSE_LL` hook so the pet wakes on real mouse input instead
/// of polling `GetCursorPos` on a timer.
fn install_mouse_waker(ctx: &egui::Context) -> bool {
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

unsafe extern "system" fn mouse_hook_proc(
    code: i32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
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
            #[cfg(feature = "test-hooks")]
            MOUSE_EVENT_COUNT.fetch_add(1, Ordering::Relaxed);
            if let Some(ctx) = MOUSE_CONTEXT.get() {
                ctx.request_repaint();
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

fn primary_button_down() -> Option<bool> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};

    let state = unsafe { GetAsyncKeyState(VK_LBUTTON as i32) };
    Some((state as u16 & 0x8000) != 0)
}

fn secondary_button_down() -> Option<bool> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_RBUTTON};

    let state = unsafe { GetAsyncKeyState(VK_RBUTTON as i32) };
    Some((state as u16 & 0x8000) != 0)
}

/// Best-effort global cursor position.
///
/// Once the window can be `WS_EX_TRANSPARENT`, egui may stop receiving cursor
/// updates, so the cached low-level hook position (or a direct `GetCursorPos`)
/// is what keeps pixel-level click-through exact.
fn global_cursor_position() -> Option<(f64, f64)> {
    if MOUSE_HOOK.load(Ordering::Relaxed) != 0 && MOUSE_POSITION_VALID.load(Ordering::Relaxed) {
        return Some((
            MOUSE_X.load(Ordering::Relaxed) as f64,
            MOUSE_Y.load(Ordering::Relaxed) as f64,
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

/// Turn off the DWM's own show/hide transition for a window.
///
/// Windows fades a window in when it is shown, and with the accessibility
/// option "fade or slide menus into view" it slides a tool window in from a
/// screen edge as well. For the bubble that reads as "the bubble slides in
/// sideways", which fights the bubble's own fade-in, so the window itself is
/// told to appear instantly at the position we computed.
fn disable_window_transitions(hwnd: *mut core::ffi::c_void) -> bool {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
    };

    let disabled: i32 = 1;
    let result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED as u32,
            &disabled as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&disabled) as u32,
        )
    };
    // The attribute cannot be read back (`DwmGetWindowAttribute` answers
    // E_INVALIDARG for it), so callers remember whether the call was accepted.
    if result >= 0 {
        true
    } else {
        tracing::debug!(result, "cannot disable DWM window transitions");
        false
    }
}

/// Re-establish per-pixel transparency.
///
/// winit creates transparent windows by giving DWM an empty blur region, and
/// that is per-window state which a forced frame recompute can drop, so it is
/// re-applied after changing the window styles.
fn enable_transparency(window: &winit::window::Window) {
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
fn strip_frame_styles(window: &winit::window::Window) -> bool {
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
fn set_no_activate(window: &winit::window::Window) {
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
        let _ = disable_window_transitions(hwnd);
    }
}

/// Same as [`set_no_activate`] for a window identified by its title, used for
/// the bubble and menu windows that egui creates on demand. Returns how many
/// windows were changed.
/// Handles of our own top-level windows on **this thread** that carry `title`.
///
/// Only this thread's windows are considered: the subclass bookkeeping below is
/// thread-local, and even reading the title of a window whose thread is not
/// pumping messages would block this one (`GetWindowTextW` is a synchronous
/// send).
fn windows_with_title(title: &str) -> Vec<windows_sys::Win32::Foundation::HWND> {
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, TRUE};
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId,
    };

    struct Lookup<'a> {
        title: &'a str,
        windows: Vec<HWND>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let lookup = unsafe { &mut *(lparam as *mut Lookup<'_>) };
        let mut owner = 0u32;
        let thread = unsafe { GetWindowThreadProcessId(hwnd, &mut owner) };
        if owner != std::process::id() || thread != unsafe { GetCurrentThreadId() } {
            return TRUE;
        }
        let mut buffer = [0u16; 256];
        let len = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
        if len <= 0 {
            return TRUE;
        }
        let text = String::from_utf16_lossy(&buffer[..len as usize]);
        if text == lookup.title {
            lookup.windows.push(hwnd);
        }
        TRUE
    }

    let mut lookup = Lookup {
        title,
        windows: Vec::new(),
    };
    unsafe {
        EnumWindows(Some(visit), &mut lookup as *mut _ as LPARAM);
    }
    lookup.windows
}

/// Strip the frame and force the non-activating style onto our popup windows
/// (bubble, egui context menu). Returns how many windows were touched.
fn style_popup_window(title: &str) -> usize {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::{
        DwmEnableBlurBehindWindow, DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND,
    };
    use windows_sys::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE,
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_BORDER,
        WS_CAPTION, WS_DLGFRAME, WS_EX_NOACTIVATE, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP,
        WS_SYSMENU,
    };

    let windows = windows_with_title(title);
    for hwnd in &windows {
        let hwnd: HWND = *hwnd;
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
            if disable_window_transitions(hwnd) {
                #[cfg(feature = "test-hooks")]
                POPUP_TRANSITIONS_DISABLED.store(true, Ordering::Relaxed);
            }
        }
    }
    windows.len()
}

/// Disable the OS show/hide transition for our own windows that keep their
/// normal activation behaviour (the conversation window, which has to accept
/// keyboard focus). Returns how many windows were touched.
fn disable_window_animation_by_title(title: &str) -> usize {
    let windows = windows_with_title(title);
    let mut disabled = 0;
    for hwnd in &windows {
        if disable_window_transitions(*hwnd) {
            disabled += 1;
            #[cfg(feature = "test-hooks")]
            POPUP_TRANSITIONS_DISABLED.store(true, Ordering::Relaxed);
        }
    }
    disabled
}
/// Work area (physical pixels) of the monitor nearest to a physical point.
fn monitor_work_area_physical(x: i32, y: i32) -> Option<PhysicalRect> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let work = info.rcWork;
        Some(PhysicalRect::new(
            work.left,
            work.top,
            work.right - work.left,
            work.bottom - work.top,
        ))
    }
}

/// Clamp a point into the work area of the monitor it is nearest to.
///
/// Used for the native popup menu so it opens on the monitor the user clicked
/// even when the pet sits right against a screen edge.
pub(crate) fn clamp_point_to_work_area(x: i32, y: i32, inset: i32) -> (i32, i32) {
    let Some(work) = monitor_work_area_physical(x, y) else {
        return (x, y);
    };
    clamp_point_to_rect(x, y, work, inset)
}

fn clamp_point_to_rect(x: i32, y: i32, work: PhysicalRect, inset: i32) -> (i32, i32) {
    let inset = inset.max(0);
    let min_x = work.x.saturating_add(inset);
    let max_x = work.right().saturating_sub(inset).max(min_x);
    let min_y = work.y.saturating_add(inset);
    let max_y = work.bottom().saturating_sub(inset).max(min_y);
    (x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

/// Login item (`HKCU\...\Run`) used by the settings toggle.
///
/// The value name can be overridden with `PETSONA_AUTOSTART_VALUE_NAME`; the
/// smoke harness uses that to test the toggle without touching the user's real
/// `Petsona` entry.
mod autostart {
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME_ENV: &str = "PETSONA_AUTOSTART_VALUE_NAME";

    fn value_name() -> String {
        std::env::var(VALUE_NAME_ENV)
            .ok()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Petsona".to_string())
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Read the login item straight from the registry: the settings checkbox
    /// shows the real OS state, not a cached config copy.
    pub fn is_enabled() -> bool {
        let subkey = wide(RUN_KEY);
        let name = wide(&value_name());
        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut key)
                != ERROR_SUCCESS
            {
                return false;
            }
            let mut kind = 0u32;
            let mut size = 0u32;
            let status = RegQueryValueExW(
                key,
                name.as_ptr(),
                std::ptr::null(),
                &mut kind,
                std::ptr::null_mut(),
                &mut size,
            );
            RegCloseKey(key);
            status == ERROR_SUCCESS
        }
    }

    pub fn enable() -> Result<(), String> {
        let command = command_line()?;
        let subkey = wide(RUN_KEY);
        let name = wide(&value_name());
        let data: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            let status = RegCreateKeyW(HKEY_CURRENT_USER, subkey.as_ptr(), &mut key);
            if status != ERROR_SUCCESS {
                return Err(format!("无法创建开机自启注册表项（错误码 {status}）"));
            }
            let status = RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr() as *const u8,
                (data.len() * std::mem::size_of::<u16>()) as u32,
            );
            RegCloseKey(key);
            if status != ERROR_SUCCESS {
                return Err(format!("无法写入开机自启注册表项（错误码 {status}）"));
            }
        }
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        let subkey = wide(RUN_KEY);
        let name = wide(&value_name());
        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut key,
            ) != ERROR_SUCCESS
            {
                // No Run key means there is nothing to remove.
                return Ok(());
            }
            let status = RegDeleteValueW(key, name.as_ptr());
            RegCloseKey(key);
            if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                Err(format!("无法删除开机自启注册表项（错误码 {status}）"))
            }
        }
    }

    fn command_line() -> Result<String, String> {
        let exe = std::env::current_exe()
            .map_err(|error| format!("无法读取当前可执行文件路径：{error}"))?;
        Ok(format!("\"{}\"", exe.display()))
    }
}

fn clear_dwm_frame(window: &winit::window::Window) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_points_into_the_work_area() {
        let work = PhysicalRect::new(0, 0, 1920, 1040);
        assert_eq!(clamp_point_to_rect(500, 500, work, 8), (500, 500));
        assert_eq!(clamp_point_to_rect(4000, -50, work, 8), (1912, 8));
    }

    #[test]
    fn second_monitor_keeps_negative_coordinates() {
        let work = PhysicalRect::new(-1920, 0, 1920, 1040);
        assert_eq!(clamp_point_to_rect(-2000, 200, work, 8), (-1912, 200));
        assert_eq!(clamp_point_to_rect(-100, 2000, work, 8), (-100, 1032));
    }
}
