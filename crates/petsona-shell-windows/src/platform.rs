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

use petsona_app::platform::{PlatformHost, PlatformMenu, PointerSnapshot};

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
    }
}

/// Same as [`set_no_activate`] for a window identified by its title, used for
/// the bubble and menu windows that egui creates on demand. Returns how many
/// windows were changed.
fn style_popup_window(title: &str) -> usize {
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, TRUE};
    use windows_sys::Win32::Graphics::Dwm::{
        DwmEnableBlurBehindWindow, DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND,
    };
    use windows_sys::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject};
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
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
        // Only windows of this very thread are ours to restyle: the subclass
        // bookkeeping below is thread-local, and even reading the title of a
        // window whose thread is not pumping messages would block this one
        // (`GetWindowTextW` is a synchronous send).
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
        if text != lookup.title {
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
