//! Native pet overlay: a layered, non-activating Win32 window rendered on its
//! own thread with its own message pump, independent of Tauri's windows.
//!
//! Runtime-driven (M2): the frame comes from the active pet's atlas
//! (`snapshot.sprite_index` at `cell_size * scale * DPI`), positions are
//! remembered via `config.window.startPosition`, and everything is clamped to
//! the monitor work area (taskbar included). Ported from the deleted C#
//! frontend (git `6bca241`): layered presentation, 50 ms pass-through poll,
//! `HTTRANSPARENT` hit testing, bottom-centre size anchoring and the
//! idle-row-union hit mask.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use petsona_core::pet::PetState;
use petsona_runtime::commands::RuntimeCommand;
use petsona_runtime::engine::RuntimeEngine;
use petsona_runtime::snapshot::RuntimeTextField;

use crate::gdi_text;
use crate::logging::log;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush, DeleteDC,
    DeleteObject, EndPaint, FillRect, GetDC, GetMonitorInfoW, GetStockObject, InvalidateRect,
    MonitorFromPoint, MonitorFromRect, ReleaseDC, SelectObject, SetBkColor, SetTextColor,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
    DEFAULT_CHARSET, DEFAULT_GUI_FONT, DEFAULT_PITCH, DIB_RGB_COLORS, FF_DONTCARE, FW_NORMAL,
    HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, OUT_DEFAULT_PRECIS,
    PAINTSTRUCT,
};
use windows_sys::Win32::Graphics::GdiPlus::{
    FillModeWinding, GdipCreateFromHDC, GdipCreateSolidFill, GdipDeleteBrush, GdipDeleteGraphics,
    GdipFillEllipse, GdipFillPolygon, GdipSetSmoothingMode, GpBrush, GpGraphics, GpSolidFill,
    PointF, SmoothingModeAntiAlias,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, VK_ESCAPE, VK_RETURN, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CallWindowProcW, CreateWindowExW, DefWindowProcW, DispatchMessageW,
    GetCaretPos, GetClientRect, GetCursorPos, GetForegroundWindow, GetMessageW, GetWindowLongPtrW,
    GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, KillTimer,
    LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassExW, SendMessageW, SetCursor,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos, SetWindowTextW,
    SetWindowsHookExW, ShowWindow, TranslateMessage, UnhookWindowsHookEx, UpdateLayeredWindow,
    WindowFromPoint, ES_AUTOVSCROLL, ES_LEFT, ES_MULTILINE, ES_WANTRETURN, GWLP_USERDATA,
    GWL_EXSTYLE, GWL_WNDPROC, IDC_ARROW, IDC_HAND, MSG, MSLLHOOKSTRUCT, SWP_NOACTIVATE, SWP_NOSIZE,
    SWP_NOZORDER, SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE, ULW_ALPHA, WH_MOUSE_LL, WM_APP,
    WM_CTLCOLOREDIT, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCHITTEST, WM_PAINT, WM_SETCURSOR, WM_TIMER, WNDCLASSEXW,
    WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP, WS_VISIBLE,
};

const CLASS_NAME: &str = "PetsonaPetWindow";
const VISUAL_TIMER: usize = 1;
const HIT_TIMER: usize = 2;
const VISUAL_INTERVAL_MS: u32 = 33;
const HIT_INTERVAL_MS: u32 = 16;
/// Single-click delay: a second press inside this window is a double click
/// (ported from `PetWindow.ClickDelayMs`, git 6bca241).
const CLICK_TIMER: usize = 3;
const CLICK_DELAY_MS: u32 = 320;
/// Drag locomotion: re-raise every `DRAG_STATE_RESEND_MS` with a
/// `DRAG_STATE_TTL_MS` lifetime so stopping the drag falls back to idle.
const DRAG_STATE_TTL_MS: u64 = 300;
const DRAG_STATE_RESEND_MS: u128 = 80;
/// Counter-movement needed before a drag flips its running direction.
const DRAG_DIRECTION_FLIP_PX: i32 = 3;
/// Gaze ellipse: enter margin 80% / exit margin 100% of the short side, and
/// a 35% centre dead zone (ported from `GazeFilter.cs`, git 6bca241).
const GAZE_ENTER_MARGIN: f32 = 0.80;
const GAZE_EXIT_MARGIN: f32 = 1.00;
const GAZE_DEAD_ZONE: f32 = 0.35;
/// Official 16-direction mapping with angular hysteresis (ported from
/// `GazeStabilizer.cs`, git 6bca241).
const GAZE_STEP_DEGREES: f32 = 22.5;
const GAZE_HYSTERESIS_DEGREES: f32 = 7.0;
const GAZE_MIN_MOVEMENT_PX: f32 = 2.0;
const ALPHA_THRESHOLD: u8 = 13;
const DRAG_THRESHOLD: i32 = 4;
const FRAME_CACHE_LIMIT: usize = 256;
const DEFAULT_MARGIN: i32 = 24;

const HIT_CLIENT: LRESULT = 1;
const HIT_TRANSPARENT: LRESULT = -1;

const BUBBLE_CLASS_NAME: &str = "PetsonaOverlayWindow";
const BUBBLE_FADE_MS: u64 = 150;
/// 14pt at 96 DPI, matching the old System.Drawing bubble font.
const BUBBLE_FONT_PX: f32 = 14.0 * 96.0 / 72.0;
const BUBBLE_PADDING_X: f32 = 16.0;
const BUBBLE_PADDING_Y: f32 = 12.0;
const BUBBLE_MAX_TEXT_WIDTH: f32 = 300.0;
const BUBBLE_RADIUS: f32 = 12.0;
const BUBBLE_PROGRESS_HEIGHT: i32 = 2;
const BUBBLE_PROGRESS_INSET: i32 = 14;
const BUBBLE_FADE_OUT_MS: u64 = 700;
const BUBBLE_GAP: i32 = 10;
const EDGE_MARGIN: i32 = 8;

// BubblePalette.cs (ARGB), git 6bca241
const LIGHT_FILL: u32 = 0xF2FF_FFFF;
const LIGHT_BORDER: u32 = 0x4600_0000;
const LIGHT_TEXT: u32 = 0xFF20_2020;
const LIGHT_ACCENT: u32 = 0xFF00_78D4;
const LIGHT_TRACK: u32 = 0x1800_0000;
const DARK_FILL: u32 = 0xF22B_2B2B;
const DARK_BORDER: u32 = 0x46FF_FFFF;
const DARK_TEXT: u32 = 0xFFF0_F0F0;
const DARK_ACCENT: u32 = 0xFF00_99FF;
const DARK_TRACK: u32 = 0x24FF_FFFF;

const COMPOSER_CLASS_NAME: &str = "PetsonaComposerWindow";
/// The pet window never owns focus, so a WH_MOUSE_LL hook forwards wheel
/// input that happens while the cursor is over the pet: scrolling down opens
/// the composer, scrolling up closes it.
const WM_WHEEL_GESTURE: u32 = WM_APP + 2;
/// OverlayLayout.Gap: spacing between the pet and panels.
const GAP: i32 = 12;
/// OverlayLayout.ComposerMinWidth: side hysteresis threshold.
const COMPOSER_MIN_WIDTH: i32 = 280;
const COMPOSER_WIDTH: i32 = 296;
const COMPOSER_HEIGHT: i32 = 44;
const COMPOSER_SEND_SIZE: i32 = 30;
const COMPOSER_SEND_INSET: i32 = 8;
const COMPOSER_EDIT_ID: i32 = 1001;
/// Codex V2 maps the `jumping` row to the hover/playful jump. Play one jump
/// when the cursor enters the pet, with a cooldown so crossing the sprite does
/// not loop the animation.
const HOVER_JUMP_COOLDOWN_MS: u64 = 1200;
/// Accumulated same-direction wheel travel needed before the composer toggles.
/// One classic notch is 120, so this needs three notches; shorter or reversed
/// bursts are discarded so page scrolling over the pet cannot open the input.
const WHEEL_TRAVEL_REQUIRED: i32 = 360;
/// A pause longer than this between wheel events resets the accumulated travel.
const WHEEL_TRAVEL_RESET_MS: u64 = 700;
/// Posted to the pet window to open the composer (also used by test hooks).
pub const WM_OPEN_COMPOSER: u32 = WM_APP + 1;

// GDI COLORREF is 0x00BBGGRR.
const COLOR_LIGHT_BG: u32 = 0x00FB_F8F6;
const COLOR_LIGHT_BORDER: u32 = 0x00EC_E7E3;
const COLOR_LIGHT_TEXT: u32 = 0x0020_2020;
const COLOR_DARK_BG: u32 = 0x0036_2B25;
const COLOR_DARK_BORDER: u32 = 0x004C_413B;
const COLOR_DARK_TEXT: u32 = 0x00F0_F0F0;

static DARK_THEME: AtomicBool = AtomicBool::new(false);

/// Called by the shell when the system/app theme changes.
pub fn set_dark_theme(dark: bool) {
    DARK_THEME.store(dark, Ordering::Relaxed);
}

struct Frame {
    width: i32,
    height: i32,
    /// Premultiplied BGRA, top-down, ready for `UpdateLayeredWindow`.
    pixels: Vec<u8>,
    mask: Vec<bool>,
}

struct Atlas {
    path: String,
    pixels: image::RgbaImage,
}

struct IdleUnion {
    key: (String, i32, i32),
    mask: Vec<bool>,
}

struct BubbleState {
    hovered: bool,
    /// Generation we sent `SetBubblePaused(true)` for; -1 = not paused.
    paused_generation: i64,
    generation: i64,
    fade_started: Instant,
    render_key: String,
    pixels: Vec<u8>,
    width: i32,
    height: i32,
    visible: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OverlaySide {
    Bottom,
    Left,
    Right,
}

struct ComposerState {
    hwnd: HWND,
    edit: HWND,
    font: isize,
    dwm_dark: Option<bool>,
    open: bool,
    draft: String,
    button_hovered: bool,
    focus_deadline: Option<Instant>,
    position: (i32, i32),
    bg_brush: isize,
    brush_dark: bool,
}

/// Same-direction wheel travel accumulated between composer gestures.
#[derive(Default)]
struct WheelTravel {
    accumulated: i32,
    last_at: Option<Instant>,
}

struct State {
    engine: Arc<Mutex<RuntimeEngine>>,
    pet_hwnd: HWND,
    bubble_hwnd: HWND,
    composer: ComposerState,
    last_side: Option<OverlaySide>,
    composer_side: Option<OverlaySide>,
    bubble: BubbleState,
    atlas: Option<Atlas>,
    frames: HashMap<(u32, i32, i32), Frame>,
    idle: Option<IdleUnion>,
    /// Last presented frame key: (sprite_index, width, height).
    rendered: Option<(u32, i32, i32)>,
    visible: bool,
    position_applied: bool,
    dragging: bool,
    moved: bool,
    drag_cursor: (i32, i32),
    drag_offset: (i32, i32),
    last_move_cursor: (i32, i32),
    drag_direction: Option<PetState>,
    drag_direction_accumulator: i32,
    drag_state_sent_at: Option<Instant>,
    last_click_at: Option<Instant>,
    transparent: Option<bool>,
    gaze_active: bool,
    gaze_direction: i32,
    gaze_last: (f32, f32),
    hover_active: bool,
    hover_last_jump: Option<Instant>,
    wheel: WheelTravel,
    fault_reported: bool,
    fault_message: Option<String>,
    fault_until: Option<Instant>,
}

thread_local! {
    static STATE: RefCell<Option<Box<State>>> = const { RefCell::new(None) };
}

static PET_CLASS: OnceLock<bool> = OnceLock::new();
static BUBBLE_CLASS: OnceLock<bool> = OnceLock::new();
static COMPOSER_CLASS: OnceLock<bool> = OnceLock::new();
static PET_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static MOUSE_HOOK: AtomicIsize = AtomicIsize::new(0);

/// Spawns the overlay thread that owns the pet window and its message pump.
pub fn spawn(engine: Arc<Mutex<RuntimeEngine>>) {
    let spawned = std::thread::Builder::new()
        .name("petsona-overlay".into())
        .spawn(move || unsafe { thread_main(engine) });
    if let Err(err) = spawned {
        log(&format!("overlay: thread spawn failed: {err}"));
    }
}

unsafe fn thread_main(engine: Arc<Mutex<RuntimeEngine>>) {
    if !register_class(CLASS_NAME, &PET_CLASS)
        || !register_class(BUBBLE_CLASS_NAME, &BUBBLE_CLASS)
        || !register_class(COMPOSER_CLASS_NAME, &COMPOSER_CLASS)
    {
        log("overlay: RegisterClassExW failed");
        return;
    }

    let hwnd = create_window(CLASS_NAME, "Petsona");
    if hwnd.is_null() {
        log("overlay: CreateWindowExW failed");
        return;
    }
    apply_dwm_attributes(hwnd);
    let bubble_hwnd = create_window(BUBBLE_CLASS_NAME, "Petsona");
    apply_dwm_attributes(bubble_hwnd);
    let composer_hwnd = create_composer_window();
    apply_composer_dwm(composer_hwnd, DARK_THEME.load(Ordering::Relaxed));
    let composer_edit = if composer_hwnd.is_null() {
        ptr::null_mut()
    } else {
        let edit = create_composer_edit(composer_hwnd);
        if !edit.is_null() {
            let old = SetWindowLongPtrW(
                edit,
                GWL_WNDPROC,
                edit_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT
                    as usize as isize,
            );
            SetWindowLongPtrW(edit, GWLP_USERDATA, old);
        }
        edit
    };
    PET_HWND.store(hwnd as isize, Ordering::Relaxed);

    let state = Box::new(State {
        engine,
        pet_hwnd: hwnd,
        bubble_hwnd,
        last_side: None,
        composer_side: None,
        composer: ComposerState {
            hwnd: composer_hwnd,
            edit: composer_edit,
            font: 0,
            dwm_dark: None,
            open: false,
            draft: String::new(),
            button_hovered: false,
            focus_deadline: None,
            position: (i32::MIN, i32::MIN),
            bg_brush: 0,
            brush_dark: false,
        },
        bubble: BubbleState {
            hovered: false,
            paused_generation: -1,
            generation: -1,
            fade_started: Instant::now(),
            render_key: String::new(),
            pixels: Vec::new(),
            width: 0,
            height: 0,
            visible: false,
        },
        atlas: None,
        frames: HashMap::new(),
        idle: None,
        rendered: None,
        visible: false,
        position_applied: false,
        dragging: false,
        moved: false,
        drag_cursor: (0, 0),
        drag_offset: (0, 0),
        last_move_cursor: (0, 0),
        drag_direction: None,
        drag_direction_accumulator: 0,
        drag_state_sent_at: None,
        last_click_at: None,
        transparent: None,
        gaze_active: false,
        gaze_direction: -1,
        gaze_last: (0.0, 0.0),
        hover_active: false,
        hover_last_jump: None,
        wheel: WheelTravel::default(),
        fault_reported: false,
        fault_message: None,
        fault_until: None,
    });
    STATE.with(|cell| *cell.borrow_mut() = Some(state));

    SetTimer(hwnd, VISUAL_TIMER, VISUAL_INTERVAL_MS, None);
    SetTimer(hwnd, HIT_TIMER, HIT_INTERVAL_MS, None);
    let hook = SetWindowsHookExW(
        WH_MOUSE_LL,
        Some(mouse_hook_proc),
        GetModuleHandleW(ptr::null()),
        0,
    );
    if hook.is_null() {
        log("overlay: mouse wheel hook not installed");
    } else {
        MOUSE_HOOK.store(hook as isize, Ordering::Relaxed);
        log("overlay: mouse wheel hook installed");
    }
    log("overlay: pet window created (hidden until the runtime is ready)");

    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    let hook = MOUSE_HOOK.swap(0, Ordering::Relaxed);
    if hook != 0 {
        UnhookWindowsHookEx(hook as *mut c_void);
    }
    log("overlay: message loop ended");
}

unsafe fn register_class(name: &str, cell: &'static OnceLock<bool>) -> bool {
    *cell.get_or_init(|| {
        let class_w = wide(name);
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(ptr::null()),
            hIcon: ptr::null_mut(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_w.as_ptr(),
            hIconSm: ptr::null_mut(),
        };
        RegisterClassExW(&wc) != 0
    })
}

unsafe fn create_window(class_name: &str, title: &str) -> HWND {
    let class_w = wide(class_name);
    let title_w = wide(title);
    CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
        class_w.as_ptr(),
        title_w.as_ptr(),
        WS_POPUP,
        0,
        0,
        1,
        1,
        ptr::null_mut(),
        ptr::null_mut(),
        GetModuleHandleW(ptr::null()),
        ptr::null(),
    )
}

unsafe fn apply_dwm_attributes(hwnd: HWND) {
    // Win11 draws a system border / rounds top-level windows unless disabled.
    let border_none: u32 = 0xFFFF_FFFE; // DWMWA_COLOR_NONE
    let dont_round: u32 = 1; // DWMWCP_DONOTROUND
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_BORDER_COLOR as u32,
        &border_none as *const _ as *const c_void,
        std::mem::size_of::<u32>() as u32,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE as u32,
        &dont_round as *const _ as *const c_void,
        std::mem::size_of::<u32>() as u32,
    );
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let handled: Option<LRESULT> = STATE.with(|cell| {
        // Win32 calls such as SetWindowLongPtrW/SetWindowPos can dispatch
        // messages synchronously back into this window procedure while an
        // outer handler still holds the borrow. Re-entrant messages are
        // handled by DefWindowProc instead of panicking (a panic across
        // extern "system" aborts the whole process).
        let mut guard = match cell.try_borrow_mut() {
            Ok(guard) => guard,
            Err(_) => return None,
        };
        let state = match guard.as_mut() {
            Some(state) => state,
            None => return None,
        };
        if !state.bubble_hwnd.is_null() && hwnd == state.bubble_hwnd {
            return match msg {
                WM_SETCURSOR => {
                    SetCursor(LoadCursorW(ptr::null_mut(), IDC_ARROW));
                    Some(1)
                }
                WM_LBUTTONUP => {
                    open_composer(state);
                    Some(0)
                }
                WM_DESTROY => Some(0),
                _ => None,
            };
        }
        if !state.composer.hwnd.is_null() && hwnd == state.composer.hwnd {
            return composer_wnd_proc_msg(hwnd, msg, wparam, lparam, state);
        }
        match msg {
            WM_TIMER => {
                if wparam == VISUAL_TIMER {
                    refresh(hwnd, state);
                } else if wparam == HIT_TIMER {
                    update_pass_through(hwnd, state);
                    update_gaze(hwnd, state);
                    update_hover(hwnd, state);
                } else if wparam == CLICK_TIMER {
                    on_click_timer(hwnd, state);
                }
                Some(0)
            }
            WM_MOUSEMOVE => {
                on_mouse_move(hwnd, state);
                Some(0)
            }
            WM_LBUTTONDOWN => {
                on_lbutton_down(hwnd, state);
                Some(0)
            }
            WM_LBUTTONUP => {
                on_lbutton_up(hwnd, state);
                Some(0)
            }
            WM_NCHITTEST => Some(on_nchittest(hwnd, state, lparam)),
            WM_OPEN_COMPOSER => {
                open_composer(state);
                Some(0)
            }
            WM_WHEEL_GESTURE => {
                if !state.dragging {
                    let delta = wparam as u16 as i16 as i32;
                    match wheel_travel_action(&mut state.wheel, delta, Instant::now()) {
                        Some(WheelAction::Open) if !state.composer.open => {
                            open_composer(state);
                            log("composer: opened by wheel travel");
                        }
                        Some(WheelAction::Close) if state.composer.open => {
                            close_composer(state);
                            log("composer: closed by wheel travel");
                        }
                        _ => {}
                    }
                }
                Some(0)
            }
            WM_SETCURSOR => {
                SetCursor(LoadCursorW(ptr::null_mut(), IDC_ARROW));
                Some(1)
            }
            WM_DESTROY => {
                KillTimer(hwnd, VISUAL_TIMER);
                KillTimer(hwnd, HIT_TIMER);
                KillTimer(hwnd, CLICK_TIMER);
                PostQuitMessage(0);
                Some(0)
            }
            _ => None,
        }
    });

    match handled {
        Some(value) => value,
        None => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Reads the runtime snapshot and shows/hides/renders the pet window.
unsafe fn refresh(hwnd: HWND, state: &mut State) {
    let (snapshot, atlas_path, error_text) = {
        let Ok(engine) = state.engine.lock() else {
            return;
        };
        (
            engine.snapshot(),
            engine.text(RuntimeTextField::AtlasPath),
            engine.text(RuntimeTextField::Error),
        )
    };

    if snapshot.faulted && !state.fault_reported {
        state.fault_reported = true;
        let lowercase = error_text.to_lowercase();
        let lock_conflict = lowercase.contains("lock");
        let message = if lock_conflict {
            "已有一个 Petsona 实例在使用同一数据目录，本窗口将在 3 秒后退出。".to_string()
        } else if error_text.trim().is_empty() {
            "Petsona 引擎发生故障，已停止处理命令。".to_string()
        } else {
            format!("Petsona 无法继续：\n{error_text}")
        };
        state.fault_message = Some(message);
        state.fault_until = Some(Instant::now() + std::time::Duration::from_secs(60));
        log(&format!("overlay: runtime faulted (lock={lock_conflict})"));
        if lock_conflict {
            // A stray second instance must not linger on the desktop.
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(3));
                std::process::exit(0);
            });
        }
    }

    let want_visible =
        snapshot.ready && snapshot.has_pet && snapshot.pet_visible && !atlas_path.is_empty();

    if want_visible != state.visible {
        if want_visible {
            if !render_frame(hwnd, state, &snapshot, &atlas_path) {
                update_bubble(state); // keeps a stale bubble from surviving
                return; // stay hidden and retry on the next tick
            }
            if !state.position_applied {
                apply_remembered_position(hwnd, state);
                state.position_applied = true;
            }
            state.visible = true;
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            log("overlay: pet shown");
        } else {
            state.visible = false;
            ShowWindow(hwnd, SW_HIDE);
            log("overlay: pet hidden");
        }
    } else if state.visible {
        render_frame(hwnd, state, &snapshot, &atlas_path);
    }
    update_bubble(state);
    update_composer(state);
}

/// Renders the current runtime frame; returns false when the atlas could not
/// be loaded (the caller keeps the window hidden).
unsafe fn render_frame(
    hwnd: HWND,
    state: &mut State,
    snapshot: &petsona_runtime::snapshot::RuntimeSnapshot,
    atlas_path: &str,
) -> bool {
    if state.atlas.as_ref().map(|atlas| atlas.path.as_str()) != Some(atlas_path) {
        match load_atlas(atlas_path) {
            Ok(atlas) => {
                state.atlas = Some(atlas);
                state.frames.clear();
                state.idle = None;
                state.rendered = None;
            }
            Err(error) => {
                log(&format!(
                    "overlay: cannot load atlas '{atlas_path}': {error}"
                ));
                return false;
            }
        }
    }

    let cell_width = snapshot.cell_width.max(1) as f32;
    let cell_height = snapshot.cell_height.max(1) as f32;
    let dpi_scale = (GetDpiForWindow(hwnd) as f32 / 96.0).max(1.0);
    let user_scale = if snapshot.scale > 0.0 {
        snapshot.scale
    } else {
        1.0
    };
    let width = (cell_width * user_scale * dpi_scale).round().max(1.0) as i32;
    let height = (cell_height * user_scale * dpi_scale).round().max(1.0) as i32;

    let key = (snapshot.sprite_index, width, height);
    if state.rendered == Some(key) {
        return true;
    }

    ensure_idle_union(state, snapshot, atlas_path, width, height);

    if !state.frames.contains_key(&key) {
        let frame = {
            let Some(atlas) = state.atlas.as_ref() else {
                return false;
            };
            build_frame(atlas, snapshot, width, height)
        };
        if state.frames.len() >= FRAME_CACHE_LIMIT {
            state.frames.clear();
        }
        state.frames.insert(key, frame);
    }

    if let Some(previous) = state.rendered {
        if previous.1 != width || previous.2 != height {
            anchor_and_move(hwnd, width, height);
        }
    }

    if let Some(frame) = state.frames.get(&key) {
        present(hwnd, frame, 255);
    }
    state.rendered = Some(key);
    true
}

/// Rebuilds the idle-row mask union whenever the atlas or the render size
/// changes; a gaze pose must not make the resting body unclickable.
fn ensure_idle_union(
    state: &mut State,
    snapshot: &petsona_runtime::snapshot::RuntimeSnapshot,
    atlas_path: &str,
    width: i32,
    height: i32,
) {
    let key = (atlas_path.to_string(), width, height);
    if state
        .idle
        .as_ref()
        .map(|idle| idle.key == key)
        .unwrap_or(false)
    {
        return;
    }
    let Some(atlas) = state.atlas.as_ref() else {
        return;
    };
    let cell_width = snapshot.cell_width.max(1);
    let cell_height = snapshot.cell_height.max(1);
    let columns = (atlas.pixels.width() / cell_width).max(1);
    let mut union = vec![false; (width * height) as usize];
    for column in 0..columns {
        if let Some(mask) = build_cell_mask(
            atlas,
            column * cell_width,
            0,
            cell_width,
            cell_height,
            width,
            height,
        ) {
            for (index, value) in mask.iter().enumerate() {
                union[index] |= *value;
            }
        }
    }
    state.idle = Some(IdleUnion { key, mask: union });
}

fn build_frame(
    atlas: &Atlas,
    snapshot: &petsona_runtime::snapshot::RuntimeSnapshot,
    width: i32,
    height: i32,
) -> Frame {
    let cell_width = snapshot.cell_width.max(1);
    let cell_height = snapshot.cell_height.max(1);
    let columns = (atlas.pixels.width() / cell_width).max(1);
    let column = snapshot.sprite_index % columns;
    let row = snapshot.sprite_index / columns;
    build_cell(
        atlas,
        column * cell_width,
        row * cell_height,
        cell_width,
        cell_height,
        width,
        height,
    )
    .unwrap_or_else(|| Frame {
        width,
        height,
        pixels: vec![0; (width * height * 4) as usize],
        mask: vec![false; (width * height) as usize],
    })
}

fn build_cell(
    atlas: &Atlas,
    x: u32,
    y: u32,
    cell_width: u32,
    cell_height: u32,
    width: i32,
    height: i32,
) -> Option<Frame> {
    let source = atlas.pixels.dimensions();
    if x >= source.0 || y >= source.1 {
        return None;
    }
    let cell_w = cell_width.min(source.0 - x).max(1);
    let cell_h = cell_height.min(source.1 - y).max(1);
    let cell = image::imageops::crop_imm(&atlas.pixels, x, y, cell_w, cell_h).to_image();
    let scaled = image::imageops::resize(
        &cell,
        width as u32,
        height as u32,
        image::imageops::FilterType::CatmullRom,
    );
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    let mut mask = Vec::with_capacity((width * height) as usize);
    for pixel in scaled.pixels() {
        let [r, g, b, a] = pixel.0;
        let alpha = a as u16;
        pixels.push((b as u16 * alpha / 255) as u8);
        pixels.push((g as u16 * alpha / 255) as u8);
        pixels.push((r as u16 * alpha / 255) as u8);
        pixels.push(a);
        mask.push(a > ALPHA_THRESHOLD);
    }
    Some(Frame {
        width,
        height,
        pixels,
        mask,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_cell_mask(
    atlas: &Atlas,
    x: u32,
    y: u32,
    cell_width: u32,
    cell_height: u32,
    width: i32,
    height: i32,
) -> Option<Vec<bool>> {
    build_cell(atlas, x, y, cell_width, cell_height, width, height).map(|frame| frame.mask)
}

unsafe fn load_atlas(path: &str) -> Result<Atlas, String> {
    let image = image::open(path).map_err(|error| error.to_string())?;
    Ok(Atlas {
        path: path.to_string(),
        pixels: image.to_rgba8(),
    })
}

/// Moves the window so its bottom-centre stays put when the frame size changes.
unsafe fn anchor_and_move(hwnd: HWND, width: i32, height: i32) {
    let mut rect: RECT = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return;
    }
    let old_width = rect.right - rect.left;
    let old_height = rect.bottom - rect.top;
    if old_width <= 1 || old_height <= 1 {
        return;
    }
    let centre_x = rect.left + old_width / 2;
    let bottom = rect.bottom;
    let desired = RECT {
        left: centre_x - width / 2,
        top: bottom - height,
        right: centre_x - width / 2 + width,
        bottom,
    };
    let clamped = clamp_to_work_area(desired);
    SetWindowPos(
        hwnd,
        ptr::null_mut(),
        clamped.left,
        clamped.top,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

/// Applies `config.window.startPosition` (physical pixels); falls back to the
/// bottom-right corner of the work area, then clamps to the visible area.
unsafe fn apply_remembered_position(hwnd: HWND, state: &State) {
    let saved = {
        let Ok(engine) = state.engine.lock() else {
            return;
        };
        engine.text(RuntimeTextField::Position)
    };
    let mut rect: RECT = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return;
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    let (x, y) = match parse_position(&saved) {
        Some((x, y)) if x > -100_000.0 && y > -100_000.0 => (x.round() as i32, y.round() as i32),
        _ => default_position(width, height),
    };
    let desired = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    };
    let clamped = clamp_to_work_area(desired);
    SetWindowPos(
        hwnd,
        ptr::null_mut(),
        clamped.left,
        clamped.top,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

unsafe fn default_position(width: i32, height: i32) -> (i32, i32) {
    let rect = primary_work_area();
    (
        rect.right - width - DEFAULT_MARGIN,
        rect.bottom - height - DEFAULT_MARGIN,
    )
}

fn parse_position(text: &str) -> Option<(f32, f32)> {
    let (x, y) = text.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

unsafe fn primary_work_area() -> RECT {
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
    if monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
        return RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
    }
    info.rcWork
}

/// Clamps a rectangle to the work area of the monitor it overlaps the most.
unsafe fn clamp_to_work_area(rect: RECT) -> RECT {
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    let monitor = MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST);
    if monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
        return rect;
    }
    let work = info.rcWork;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    let max_left = (work.right - width).max(work.left);
    let max_top = (work.bottom - height).max(work.top);
    let left = rect.left.clamp(work.left, max_left);
    let top = rect.top.clamp(work.top, max_top);
    RECT {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

unsafe fn present(hwnd: HWND, frame: &Frame, opacity: u8) {
    present_pixels(hwnd, &frame.pixels, frame.width, frame.height, opacity);
}

unsafe fn present_pixels(hwnd: HWND, pixels: &[u8], width: i32, height: i32, opacity: u8) {
    let screen_dc = GetDC(ptr::null_mut());
    if screen_dc.is_null() {
        return;
    }
    let mem_dc = CreateCompatibleDC(screen_dc);
    if mem_dc.is_null() {
        ReleaseDC(ptr::null_mut(), screen_dc);
        return;
    }

    let mut info: BITMAPINFO = std::mem::zeroed();
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height, // top-down
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        ..std::mem::zeroed()
    };

    let mut bits: *mut c_void = ptr::null_mut();
    let dib = CreateDIBSection(mem_dc, &info, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
    if dib.is_null() || bits.is_null() {
        DeleteDC(mem_dc);
        ReleaseDC(ptr::null_mut(), screen_dc);
        return;
    }

    let old = SelectObject(mem_dc, dib as HGDIOBJ);
    ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());

    let size = SIZE {
        cx: width,
        cy: height,
    };
    let src = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: 0, // AC_SRC_OVER
        BlendFlags: 0,
        SourceConstantAlpha: opacity,
        AlphaFormat: 1, // AC_SRC_ALPHA
    };
    let _ = UpdateLayeredWindow(
        hwnd,
        screen_dc,
        ptr::null(),
        &size,
        mem_dc,
        &src,
        0,
        &blend,
        ULW_ALPHA,
    );

    SelectObject(mem_dc, old);
    DeleteObject(dib as HGDIOBJ);
    DeleteDC(mem_dc);
    ReleaseDC(ptr::null_mut(), screen_dc);
}

fn raise_pet_state(state: &State, pet_state: PetState) {
    if let Ok(engine) = state.engine.lock() {
        let _ = engine.send(RuntimeCommand::SetState {
            state: pet_state,
            ttl: None,
        });
    }
}

fn send_drag_state(state: &mut State, direction: PetState) {
    if let Ok(engine) = state.engine.lock() {
        let _ = engine.send(RuntimeCommand::SetState {
            state: direction,
            ttl: Some(Duration::from_millis(DRAG_STATE_TTL_MS)),
        });
    }
    state.drag_direction = Some(direction);
    state.drag_direction_accumulator = 0;
    state.drag_state_sent_at = Some(Instant::now());
}

fn reset_drag_state(state: &mut State) {
    if state.drag_direction.take().is_some() {
        if let Ok(engine) = state.engine.lock() {
            let _ = engine.send(RuntimeCommand::SetState {
                state: PetState::Idle,
                ttl: Some(Duration::from_millis(1)),
            });
        }
    }
    state.drag_direction_accumulator = 0;
    state.drag_state_sent_at = None;
}

/// Direction update for drag locomotion: the latest horizontal step wins, but
/// a flip needs `DRAG_DIRECTION_FLIP_PX` of accumulated counter movement so
/// hand jitter cannot flap the pose (ported from `PetWindow.cs`).
fn next_drag_direction(
    current: Option<PetState>,
    step_x: i32,
    accumulator: &mut i32,
) -> Option<PetState> {
    if step_x == 0 {
        return None;
    }
    let candidate = if step_x > 0 {
        PetState::RunningRight
    } else {
        PetState::RunningLeft
    };
    match current {
        None => Some(candidate),
        Some(state) if state == candidate => {
            *accumulator = 0;
            None
        }
        Some(_) => {
            *accumulator += step_x.abs();
            if *accumulator >= DRAG_DIRECTION_FLIP_PX {
                *accumulator = 0;
                Some(candidate)
            } else {
                None
            }
        }
    }
}

unsafe fn on_click_timer(hwnd: HWND, state: &mut State) {
    KillTimer(hwnd, CLICK_TIMER);
    state.last_click_at = None;
    if let Ok(engine) = state.engine.lock() {
        let _ = engine.send(RuntimeCommand::SetState {
            state: PetState::Waving,
            ttl: None,
        });
        let _ = engine.send(RuntimeCommand::ShowBubble {
            text: "你好，我在这里".to_string(),
            ttl: Duration::from_secs(5),
        });
    }
    log("pet: single click -> waving + bubble");
}

unsafe fn on_lbutton_down(hwnd: HWND, state: &mut State) {
    // Old semantics: a second press inside the click window is a double click;
    // it cancels the pending single click and never starts a drag.
    if state
        .last_click_at
        .is_some_and(|at| at.elapsed() <= Duration::from_millis(u64::from(CLICK_DELAY_MS)))
    {
        state.last_click_at = None;
        KillTimer(hwnd, CLICK_TIMER);
        raise_pet_state(state, PetState::Jumping);
        log("pet: double click -> jumping");
        return;
    }

    let mut cursor: POINT = std::mem::zeroed();
    if GetCursorPos(&mut cursor) == 0 {
        return;
    }
    let mut rect: RECT = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return;
    }
    state.dragging = true;
    state.moved = false;
    state.drag_cursor = (cursor.x, cursor.y);
    state.last_move_cursor = (cursor.x, cursor.y);
    state.drag_offset = (cursor.x - rect.left, cursor.y - rect.top);
    state.drag_direction = None;
    state.drag_direction_accumulator = 0;
    state.drag_state_sent_at = None;
    state.last_click_at = Some(Instant::now());
    SetCapture(hwnd);
    SetTimer(hwnd, CLICK_TIMER, CLICK_DELAY_MS, None);
    log(&format!(
        "pet: mouse down cursor=({},{}) rect=({},{})",
        cursor.x, cursor.y, rect.left, rect.top
    ));
}

unsafe fn on_mouse_move(hwnd: HWND, state: &mut State) {
    if !state.dragging {
        return;
    }
    let mut cursor: POINT = std::mem::zeroed();
    if GetCursorPos(&mut cursor) == 0 {
        return;
    }
    let dx = cursor.x - state.drag_cursor.0;
    let dy = cursor.y - state.drag_cursor.1;
    if !state.moved && (dx.abs() + dy.abs()) >= DRAG_THRESHOLD {
        state.moved = true;
        // A real drag cancels the pending single click.
        KillTimer(hwnd, CLICK_TIMER);
        state.last_click_at = None;
        state.last_move_cursor = (cursor.x, cursor.y);
        log("pet: drag started");
    }

    let mut rect: RECT = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return;
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    let desired = RECT {
        left: cursor.x - state.drag_offset.0,
        top: cursor.y - state.drag_offset.1,
        right: cursor.x - state.drag_offset.0 + width,
        bottom: cursor.y - state.drag_offset.1 + height,
    };
    let clamped = clamp_to_work_area(desired);
    SetWindowPos(
        hwnd,
        ptr::null_mut(),
        clamped.left,
        clamped.top,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
    update_composer(state);

    if !state.moved {
        return;
    }
    let step_x = cursor.x - state.last_move_cursor.0;
    state.last_move_cursor = (cursor.x, cursor.y);
    if let Some(direction) = next_drag_direction(
        state.drag_direction,
        step_x,
        &mut state.drag_direction_accumulator,
    ) {
        send_drag_state(state, direction);
    }
    if let (Some(direction), Some(sent_at)) = (state.drag_direction, state.drag_state_sent_at) {
        if sent_at.elapsed().as_millis() >= DRAG_STATE_RESEND_MS {
            send_drag_state(state, direction);
        }
    }
}

unsafe fn on_lbutton_up(hwnd: HWND, state: &mut State) {
    if !state.dragging {
        return;
    }
    ReleaseCapture();
    state.dragging = false;
    if state.moved {
        let mut rect: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut rect) != 0 {
            if let Ok(engine) = state.engine.lock() {
                let _ = engine.send(RuntimeCommand::SetPosition {
                    x: rect.left as f32,
                    y: rect.top as f32,
                });
            }
            log(&format!("pet: drag ended at ({},{})", rect.left, rect.top));
        }
        reset_drag_state(state);
    } else {
        log("pet: click pending");
    }
}

unsafe fn on_nchittest(hwnd: HWND, state: &State, lparam: LPARAM) -> LRESULT {
    let value = lparam as u64;
    let x = ((value & 0xFFFF) as u16 as i16) as i32;
    let y = (((value >> 16) & 0xFFFF) as u16 as i16) as i32;
    let mut rect: RECT = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return HIT_CLIENT;
    }
    if hit(state, x - rect.left, y - rect.top) {
        HIT_CLIENT
    } else {
        HIT_TRANSPARENT
    }
}

/// Samples the global cursor at 16 ms and drives the runtime gaze target with
/// the stabilized official direction. Mirrors `AppController.SampleGaze`.
/// Screen position of the edit caret, falling back to the field centre while
/// the caret is not materialised yet.
unsafe fn composer_caret_position(edit: HWND) -> Option<(f32, f32)> {
    if edit.is_null() {
        return None;
    }
    let mut rect: RECT = std::mem::zeroed();
    if GetWindowRect(edit, &mut rect) == 0 {
        return None;
    }
    let mut caret: POINT = std::mem::zeroed();
    if GetCaretPos(&mut caret) == 0 {
        return Some((
            (rect.left + rect.right) as f32 / 2.0,
            (rect.top + rect.bottom) as f32 / 2.0,
        ));
    }
    // GetCaretPos is in edit client coordinates; the edit has no border, so
    // the window origin maps directly. +8 aims at the middle of the text line.
    Some((
        rect.left as f32 + caret.x as f32 + 2.0,
        rect.top as f32 + caret.y as f32 + 8.0,
    ))
}

unsafe fn update_gaze(hwnd: HWND, state: &mut State) {
    if !state.visible || state.dragging || state.fault_reported {
        clear_gaze(state);
        return;
    }
    let Some((_, width, height)) = state.rendered else {
        clear_gaze(state);
        return;
    };

    // While the composer is focused the pet watches the text caret so typing
    // feels attended. The caret sits outside the gaze ellipse, so this branch
    // bypasses the enter/exit margins and always drives the pose.
    if state.composer.open && GetForegroundWindow() == state.composer.hwnd {
        if let Some((caret_x, caret_y)) = composer_caret_position(state.composer.edit) {
            let mut rect: RECT = std::mem::zeroed();
            if GetWindowRect(hwnd, &mut rect) != 0 {
                let dx = caret_x - (rect.left as f32 + width as f32 / 2.0);
                let dy = caret_y - (rect.top as f32 + height as f32 / 2.0);
                let direction = gaze_stabilize(state, dx, dy);
                let (unit_x, unit_y) = gaze_unit_vector(direction);
                if let Ok(engine) = state.engine.lock() {
                    let _ = engine.send(RuntimeCommand::SetGazeTarget {
                        dx: unit_x,
                        dy: unit_y,
                    });
                }
                state.gaze_active = true;
                return;
            }
        }
    }

    let mut cursor: POINT = std::mem::zeroed();
    if GetCursorPos(&mut cursor) == 0 {
        return;
    }
    let mut rect: RECT = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return;
    }

    let width = width as f32;
    let height = height as f32;
    let dx = cursor.x as f32 - (rect.left as f32 + width / 2.0);
    let dy = cursor.y as f32 - (rect.top as f32 + height / 2.0);

    if !gaze_inside(dx, dy, width, height, state.gaze_active)
        || gaze_in_dead_zone(dx, dy, width, height)
    {
        clear_gaze(state);
        return;
    }

    let direction = gaze_stabilize(state, dx, dy);
    let (unit_x, unit_y) = gaze_unit_vector(direction);
    if let Ok(engine) = state.engine.lock() {
        let _ = engine.send(RuntimeCommand::SetGazeTarget {
            dx: unit_x,
            dy: unit_y,
        });
    }
    state.gaze_active = true;
}

/// Sends `ClearGaze` only when a target is currently held, then resets the
/// stabilizer (a fresh approach must not inherit stale hysteresis).
fn clear_gaze(state: &mut State) {
    if state.gaze_active {
        if let Ok(engine) = state.engine.lock() {
            let _ = engine.send(RuntimeCommand::ClearGaze);
        }
        state.gaze_active = false;
    }
    state.gaze_direction = -1;
    state.gaze_last = (0.0, 0.0);
}

fn gaze_inside(dx: f32, dy: f32, width: f32, height: f32, was_active: bool) -> bool {
    let short = width.min(height);
    let margin = short
        * if was_active {
            GAZE_EXIT_MARGIN
        } else {
            GAZE_ENTER_MARGIN
        };
    let radius_x = width * 0.5 + margin;
    let radius_y = height * 0.5 + margin;
    if radius_x <= 0.0 || radius_y <= 0.0 {
        return false;
    }
    (dx * dx) / (radius_x * radius_x) + (dy * dy) / (radius_y * radius_y) <= 1.0
}

fn gaze_in_dead_zone(dx: f32, dy: f32, width: f32, height: f32) -> bool {
    let dead_zone = width.min(height) * GAZE_DEAD_ZONE;
    (dx * dx + dy * dy).sqrt() <= dead_zone
}

fn gaze_stabilize(state: &mut State, dx: f32, dy: f32) -> i32 {
    let raw = gaze_quantize(dx, dy);
    if state.gaze_direction < 0 {
        state.gaze_direction = raw;
        state.gaze_last = (dx, dy);
        return raw;
    }

    let move_x = dx - state.gaze_last.0;
    let move_y = dy - state.gaze_last.1;
    if move_x * move_x + move_y * move_y < GAZE_MIN_MOVEMENT_PX * GAZE_MIN_MOVEMENT_PX {
        return state.gaze_direction;
    }
    state.gaze_last = (dx, dy);

    let held_angle = state.gaze_direction as f32 * GAZE_STEP_DEGREES;
    let delta = normalize_signed(gaze_angle_degrees(dx, dy) - held_angle);
    if delta.abs() > GAZE_STEP_DEGREES / 2.0 + GAZE_HYSTERESIS_DEGREES {
        state.gaze_direction = raw;
    }
    state.gaze_direction
}

fn gaze_angle_degrees(dx: f32, dy: f32) -> f32 {
    (dx.atan2(-dy).to_degrees() + 360.0) % 360.0
}

fn gaze_quantize(dx: f32, dy: f32) -> i32 {
    ((gaze_angle_degrees(dx, dy) / GAZE_STEP_DEGREES).round() as i32).rem_euclid(16)
}

fn gaze_unit_vector(direction: i32) -> (f32, f32) {
    let normalized = direction.rem_euclid(16) as f32;
    let radians = normalized * GAZE_STEP_DEGREES * std::f32::consts::PI / 180.0;
    (radians.sin(), -radians.cos())
}

fn normalize_signed(degrees: f32) -> f32 {
    let mut value = degrees % 360.0;
    if value > 180.0 {
        value -= 360.0;
    } else if value < -180.0 {
        value += 360.0;
    }
    value
}

/// 16 ms poll toggling `WS_EX_TRANSPARENT` so transparent pixels pass clicks
/// through to whatever is underneath (cross-process HTTRANSPARENT alone is not
/// reliable).
unsafe fn update_pass_through(hwnd: HWND, state: &mut State) {
    if !state.visible {
        return;
    }
    let mut cursor: POINT = std::mem::zeroed();
    if GetCursorPos(&mut cursor) == 0 {
        return;
    }
    let mut rect: RECT = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return;
    }
    let want_transparent = !hit(state, cursor.x - rect.left, cursor.y - rect.top);
    if state.transparent == Some(want_transparent) {
        return;
    }
    let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    let updated = if want_transparent {
        style | WS_EX_TRANSPARENT
    } else {
        style & !WS_EX_TRANSPARENT
    };
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, updated as isize);
    state.transparent = Some(want_transparent);
}

/// 16 ms hover reaction: entering the pet's opaque pixels plays one jump.
/// Dragging, the pending single-click window and an open composer suppress it,
/// and a cooldown keeps re-entry from looping the animation.
unsafe fn update_hover(hwnd: HWND, state: &mut State) {
    if !state.visible || state.fault_reported {
        state.hover_active = false;
        return;
    }
    let mut cursor: POINT = std::mem::zeroed();
    if GetCursorPos(&mut cursor) == 0 {
        return;
    }
    let mut rect: RECT = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return;
    }
    let inside = hit(state, cursor.x - rect.left, cursor.y - rect.top);
    let now = Instant::now();
    let cooldown_ready = state.hover_last_jump.map_or(true, |at| {
        now.duration_since(at) >= Duration::from_millis(HOVER_JUMP_COOLDOWN_MS)
    });
    let should_jump = hover_should_jump(
        state.hover_active,
        inside,
        state.dragging,
        state.composer.open,
        state.last_click_at.is_some(),
        cooldown_ready,
    );
    state.hover_active = inside;
    if !should_jump {
        return;
    }
    state.hover_last_jump = Some(now);
    if let Ok(engine) = state.engine.lock() {
        let _ = engine.send(RuntimeCommand::SetState {
            state: PetState::Jumping,
            ttl: None,
        });
    }
    log("pet: hover -> jumping");
}

fn hit(state: &State, x: i32, y: i32) -> bool {
    let Some((index, width, height)) = state.rendered else {
        return false;
    };
    if x < 0 || y < 0 || x >= width || y >= height {
        return false;
    }
    let offset = (y as usize) * (width as usize) + (x as usize);
    let frame_hit = state
        .frames
        .get(&(index, width, height))
        .and_then(|frame| frame.mask.get(offset))
        .copied()
        .unwrap_or(false);
    let idle_hit = state
        .idle
        .as_ref()
        .and_then(|idle| idle.mask.get(offset))
        .copied()
        .unwrap_or(false);
    frame_hit || idle_hit
}

/// Closes the composer while keeping the draft (also used by the Esc path).
fn close_composer(state: &mut State) {
    if !state.composer.open {
        return;
    }
    state.composer.open = false;
    state.composer_side = None;
    unsafe {
        ShowWindow(state.composer.hwnd, SW_HIDE);
    }
    log("composer: closed (draft kept)");
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WheelAction {
    Open,
    Close,
}

/// Scroll down over the pet opens the composer, scroll up closes it. A single
/// event never triggers: same-direction travel must accumulate to
/// `WHEEL_TRAVEL_REQUIRED` before the pause window expires, so a one-notch
/// scroll while crossing the pet is ignored.
fn wheel_travel_action(travel: &mut WheelTravel, delta: i32, now: Instant) -> Option<WheelAction> {
    if delta == 0 {
        return None;
    }
    let stale = travel
        .last_at
        .is_some_and(|last| now.duration_since(last).as_millis() as u64 > WHEEL_TRAVEL_RESET_MS);
    if stale || travel.accumulated.signum() * delta.signum() < 0 {
        travel.accumulated = 0;
    }
    travel.accumulated += delta;
    travel.last_at = Some(now);
    if travel.accumulated <= -WHEEL_TRAVEL_REQUIRED {
        travel.accumulated = 0;
        travel.last_at = None;
        return Some(WheelAction::Open);
    }
    if travel.accumulated >= WHEEL_TRAVEL_REQUIRED {
        travel.accumulated = 0;
        travel.last_at = None;
        return Some(WheelAction::Close);
    }
    None
}

/// True when the cursor just entered the pet and the pet is free to react.
/// The Codex V2 contract maps `jumping` to the hover jump, so the overlay
/// plays it once per entry and later resumes the gaze/idle state.
fn hover_should_jump(
    was_hovering: bool,
    inside: bool,
    dragging: bool,
    composer_open: bool,
    click_pending: bool,
    cooldown_ready: bool,
) -> bool {
    inside && !was_hovering && !dragging && !composer_open && !click_pending && cooldown_ready
}

/// Global low-level wheel hook. Only wheel input that lands on the pet is
/// forwarded; everything else is passed through untouched, so normal
/// scrolling keeps working.
unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam == WM_MOUSEWHEEL as usize {
        let info = &*(lparam as *const MSLLHOOKSTRUCT);
        let pet = PET_HWND.load(Ordering::Relaxed);
        if pet != 0 && WindowFromPoint(info.pt) == pet as HWND {
            let delta = ((info.mouseData >> 16) & 0xFFFF) as u16 as i16;
            PostMessageW(pet as HWND, WM_WHEEL_GESTURE, delta as isize as usize, 0);
        }
    }
    CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
}

/// Drives the speech bubble from the runtime projection: fade-in, progress
/// bar, hover pause (polled, no injected input) and pet-relative placement.
fn update_bubble(state: &mut State) {
    if state.bubble_hwnd.is_null() {
        return;
    }

    let fault_text = state.fault_message.clone().filter(|_| {
        state
            .fault_until
            .is_some_and(|until| Instant::now() < until)
    });
    if fault_text.is_none() && (!state.visible || state.pet_hwnd.is_null()) {
        hide_bubble(state);
        return;
    }

    let (text, remaining_ms, total_ms, generation) = if let Some(message) = fault_text {
        // Locally rendered fault notice (60s), mirroring the old HandleFault.
        (message, 60_000i64, 60_000i64, -1000i64)
    } else {
        let (text, timing) = {
            let Ok(engine) = state.engine.lock() else {
                return;
            };
            (
                engine.text(RuntimeTextField::Bubble),
                engine.text(RuntimeTextField::BubbleTiming),
            )
        };
        if text.trim().is_empty() {
            hide_bubble(state);
            return;
        }
        let Some(timing) = parse_bubble_timing(&timing) else {
            hide_bubble(state);
            return;
        };
        (text, timing.0, timing.1, timing.2)
    };

    // Hover is derived from the cursor position: the bubble must pause while
    // the pointer rests on it, and resume from the remaining time after.
    let hovered = unsafe {
        let mut cursor: POINT = std::mem::zeroed();
        let mut rect: RECT = std::mem::zeroed();
        GetCursorPos(&mut cursor) != 0
            && state.bubble.visible
            && GetWindowRect(state.bubble_hwnd, &mut rect) != 0
            && cursor.x >= rect.left
            && cursor.x < rect.right
            && cursor.y >= rect.top
            && cursor.y < rect.bottom
    };
    state.bubble.hovered = hovered;

    if state.fault_reported {
        state.bubble.paused_generation = -1;
    } else if !hovered && state.bubble.paused_generation != -1 {
        if let Ok(engine) = state.engine.lock() {
            let _ = engine.send(RuntimeCommand::SetBubblePaused(false));
        }
        state.bubble.paused_generation = -1;
    }
    if !state.fault_reported && hovered && state.bubble.paused_generation != generation {
        if let Ok(engine) = state.engine.lock() {
            let _ = engine.send(RuntimeCommand::SetBubblePaused(true));
        }
        state.bubble.paused_generation = generation;
    }

    let progress = if total_ms <= 0 {
        1.0
    } else {
        (remaining_ms as f32 / total_ms as f32).clamp(0.0, 1.0)
    };
    let render_key = format!("{text}\u{1f}{}", (progress * 120.0).round() as i32);
    if render_key != state.bubble.render_key || state.bubble.pixels.is_empty() {
        match render_bubble(&text, progress) {
            Some((pixels, width, height)) => {
                state.bubble.pixels = pixels;
                state.bubble.width = width;
                state.bubble.height = height;
                state.bubble.render_key = render_key;
            }
            None => return,
        }
    }

    if generation != state.bubble.generation {
        state.bubble.generation = generation;
        state.bubble.fade_started = Instant::now();
    }
    let elapsed = state.bubble.fade_started.elapsed().as_millis() as u64;
    let fade_in = ((elapsed * 255) / BUBBLE_FADE_MS).min(255) as f32 / 255.0;
    // Fade the final stretch instead of cutting the card off abruptly; hover
    // pauses the runtime countdown, which holds this value because
    // `remaining_ms` stops decreasing while paused.
    let fade_out = if remaining_ms <= 0 || remaining_ms as u64 >= BUBBLE_FADE_OUT_MS {
        1.0
    } else {
        remaining_ms as f32 / BUBBLE_FADE_OUT_MS as f32
    };
    let opacity = ((fade_in * fade_out) * 255.0).round().clamp(0.0, 255.0) as u8;

    unsafe {
        let mut pet_rect: RECT = std::mem::zeroed();
        if GetWindowRect(state.pet_hwnd, &mut pet_rect) == 0 {
            return;
        }
        let has_frame = pet_rect.right - pet_rect.left > 1;
        let (x, y) = if has_frame {
            position_bubble(pet_rect, state.bubble.width, state.bubble.height)
        } else {
            let work = primary_work_area();
            (
                work.right - state.bubble.width - DEFAULT_MARGIN,
                work.bottom - state.bubble.height - DEFAULT_MARGIN,
            )
        };
        SetWindowPos(
            state.bubble_hwnd,
            ptr::null_mut(),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        present_pixels(
            state.bubble_hwnd,
            &state.bubble.pixels,
            state.bubble.width,
            state.bubble.height,
            opacity,
        );
        if !state.bubble.visible {
            state.bubble.visible = true;
            ShowWindow(state.bubble_hwnd, SW_SHOWNOACTIVATE);
            log("bubble: shown");
        }
    }
}

fn hide_bubble(state: &mut State) {
    if state.bubble_hwnd.is_null() {
        return;
    }
    state.bubble.hovered = false;
    state.bubble.paused_generation = -1;
    state.bubble.generation = -1;
    state.bubble.render_key.clear();
    state.bubble.pixels.clear();
    if state.bubble.visible {
        state.bubble.visible = false;
        unsafe {
            ShowWindow(state.bubble_hwnd, SW_HIDE);
        }
        log("bubble: hidden");
    }
}

fn parse_bubble_timing(text: &str) -> Option<(i64, i64, i64)> {
    let mut parts = text.split(',');
    let remaining = parts.next()?.trim().parse().ok()?;
    let total = parts.next()?.trim().parse().ok()?;
    let generation = parts.next()?.trim().parse().ok()?;
    Some((remaining, total, generation))
}

/// Paints the bubble: rounded panel + 1px border + progress bar + GDI+ text.
fn render_bubble(text: &str, progress: f32) -> Option<(Vec<u8>, i32, i32)> {
    let (text_width, text_height) =
        gdi_text::measure(text, BUBBLE_FONT_PX, BUBBLE_MAX_TEXT_WIDTH, 240.0)?;
    let width = ((text_width.ceil() as i32) + (BUBBLE_PADDING_X as i32 * 2))
        .clamp(120, BUBBLE_MAX_TEXT_WIDTH as i32 + 32);
    let height =
        text_height.ceil() as i32 + (BUBBLE_PADDING_Y as i32 * 2) + BUBBLE_PROGRESS_HEIGHT + 6;
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    let dark = DARK_THEME.load(Ordering::Relaxed);
    let (fill, border, text_color, accent, track) = if dark {
        (DARK_FILL, DARK_BORDER, DARK_TEXT, DARK_ACCENT, DARK_TRACK)
    } else {
        (
            LIGHT_FILL,
            LIGHT_BORDER,
            LIGHT_TEXT,
            LIGHT_ACCENT,
            LIGHT_TRACK,
        )
    };

    let outer = (0.5f32, 0.5f32, width as f32 - 0.5, height as f32 - 0.5);
    let inner = (1.5f32, 1.5f32, width as f32 - 1.5, height as f32 - 1.5);
    for y in 0..height {
        for x in 0..width {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let outer_coverage = rounded_rect_coverage(px, py, outer, BUBBLE_RADIUS);
            let inner_coverage =
                rounded_rect_coverage(px, py, inner, (BUBBLE_RADIUS - 1.0).max(0.0));
            let index = ((y * width + x) * 4) as usize;
            if outer_coverage > 0.0 {
                blend_argb(&mut pixels, index, border, outer_coverage);
            }
            if inner_coverage > 0.0 {
                blend_argb(&mut pixels, index, fill, inner_coverage);
            }
        }
    }

    // Hairline countdown: inset from the rounded corners, round caps, and a
    // barely-there track so the bubble reads as a card instead of a widget.
    let bar_left = BUBBLE_PROGRESS_INSET;
    let bar_right = width - BUBBLE_PROGRESS_INSET;
    let bar_top = height - BUBBLE_PROGRESS_HEIGHT - 4;
    let bar_bottom = bar_top + BUBBLE_PROGRESS_HEIGHT;
    let bar_radius = BUBBLE_PROGRESS_HEIGHT as f32 / 2.0;
    fill_rounded_rect_argb(
        &mut pixels,
        width,
        height,
        bar_left,
        bar_top,
        bar_right,
        bar_bottom,
        bar_radius,
        track,
    );
    let fill_right = bar_left + (((bar_right - bar_left) as f32) * progress).round() as i32;
    fill_rounded_rect_argb(
        &mut pixels,
        width,
        height,
        bar_left,
        bar_top,
        fill_right,
        bar_bottom,
        bar_radius,
        accent,
    );

    let layout_width = width as f32 - BUBBLE_PADDING_X * 2.0;
    let layout_height =
        height as f32 - BUBBLE_PADDING_Y * 2.0 - BUBBLE_PROGRESS_HEIGHT as f32 - 6.0;
    let _ = gdi_text::draw(
        &mut pixels,
        width,
        height,
        BUBBLE_PADDING_X,
        BUBBLE_PADDING_Y,
        layout_width,
        layout_height,
        text,
        BUBBLE_FONT_PX,
        text_color,
    );

    Some((pixels, width, height))
}

/// Signed-distance coverage for an anti-aliased rounded rectangle.
fn rounded_rect_coverage(px: f32, py: f32, rect: (f32, f32, f32, f32), radius: f32) -> f32 {
    let (left, top, right, bottom) = rect;
    let cx = (left + right) * 0.5;
    let cy = (top + bottom) * 0.5;
    let half_w = ((right - left) * 0.5 - radius).max(0.0);
    let half_h = ((bottom - top) * 0.5 - radius).max(0.0);
    let qx = (px - cx).abs() - half_w;
    let qy = (py - cy).abs() - half_h;
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    let distance = (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - radius;
    (0.5 - distance).clamp(0.0, 1.0)
}

/// Source-over blend of an ARGB colour into a premultiplied BGRA buffer.
fn blend_argb(pixels: &mut [u8], index: usize, argb: u32, coverage: f32) {
    let alpha = ((argb >> 24) & 0xFF) as f32 * coverage.clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let r = ((argb >> 16) & 0xFF) as f32;
    let g = ((argb >> 8) & 0xFF) as f32;
    let b = (argb & 0xFF) as f32;
    let inverse = 1.0 - alpha / 255.0;
    pixels[index] = ((b * alpha / 255.0) + pixels[index] as f32 * inverse)
        .round()
        .clamp(0.0, 255.0) as u8;
    pixels[index + 1] = ((g * alpha / 255.0) + pixels[index + 1] as f32 * inverse)
        .round()
        .clamp(0.0, 255.0) as u8;
    pixels[index + 2] = ((r * alpha / 255.0) + pixels[index + 2] as f32 * inverse)
        .round()
        .clamp(0.0, 255.0) as u8;
    pixels[index + 3] = (alpha + pixels[index + 3] as f32 * inverse)
        .round()
        .clamp(0.0, 255.0) as u8;
}

#[allow(clippy::too_many_arguments)]
fn fill_rounded_rect_argb(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    radius: f32,
    argb: u32,
) {
    if right <= left || bottom <= top {
        return;
    }
    let rect = (left as f32, top as f32, right as f32, bottom as f32);
    for row in top.max(0)..bottom.min(buffer_height) {
        for column in left.max(0)..right.min(buffer_width) {
            let coverage =
                rounded_rect_coverage(column as f32 + 0.5, row as f32 + 0.5, rect, radius);
            if coverage > 0.0 {
                let index = ((row * buffer_width + column) * 4) as usize;
                blend_argb(pixels, index, argb, coverage);
            }
        }
    }
}

/// Above the pet by default, below when there is no room; clamped to work.
unsafe fn position_bubble(pet: RECT, width: i32, height: i32) -> (i32, i32) {
    let pet_width = pet.right - pet.left;
    let mut x = pet.left + (pet_width - width) / 2;
    let mut y = pet.top - height - BUBBLE_GAP;

    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    let monitor = MonitorFromRect(&pet, MONITOR_DEFAULTTONEAREST);
    let work_top = if !monitor.is_null() && GetMonitorInfoW(monitor, &mut info) != 0 {
        info.rcWork.top
    } else {
        0
    };
    if y < work_top + EDGE_MARGIN {
        y = pet.bottom + BUBBLE_GAP;
    }

    let desired = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    };
    let clamped = clamp_to_work_area(desired);
    x = clamped.left;
    y = clamped.top;
    (x, y)
}

// ---------------------------------------------------------------------------
// Composer: a focusable, chrome-free popup with a native EDIT control.
// Enter sends, Shift+Enter inserts a newline, Esc closes with the draft kept.
// ---------------------------------------------------------------------------

fn with_state<R>(f: impl FnOnce(&mut State) -> R, fallback: R) -> R {
    STATE.with(|cell| {
        let mut guard = match cell.try_borrow_mut() {
            Ok(guard) => guard,
            Err(_) => return fallback,
        };
        match guard.as_mut() {
            Some(state) => f(state),
            None => fallback,
        }
    })
}

unsafe fn create_composer_window() -> HWND {
    let class_w = wide(COMPOSER_CLASS_NAME);
    let title_w = wide("Petsona");
    // Layered per-pixel surface: the rounded pill, hairline border, circular
    // send button and glyph are anti-aliased in software (the old GDI
    // RoundRect/Ellipse/Polygon path produced hard, jagged edges).
    CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        class_w.as_ptr(),
        title_w.as_ptr(),
        WS_POPUP,
        0,
        0,
        COMPOSER_WIDTH,
        COMPOSER_HEIGHT,
        ptr::null_mut(),
        ptr::null_mut(),
        GetModuleHandleW(ptr::null()),
        ptr::null(),
    )
}

unsafe fn create_composer_edit(parent: HWND) -> HWND {
    let class_w = wide("EDIT");
    let style: u32 = WS_CHILD
        | WS_VISIBLE
        | (ES_LEFT as u32)
        | (ES_MULTILINE as u32)
        | (ES_AUTOVSCROLL as u32)
        | (ES_WANTRETURN as u32);
    let edit = CreateWindowExW(
        0,
        class_w.as_ptr(),
        ptr::null(),
        style,
        16,
        12,
        COMPOSER_WIDTH - 16 - COMPOSER_SEND_SIZE - COMPOSER_SEND_INSET - 10,
        COMPOSER_HEIGHT - 24,
        parent,
        COMPOSER_EDIT_ID as isize as *mut c_void,
        GetModuleHandleW(ptr::null()),
        ptr::null(),
    );
    if !edit.is_null() {
        // WM_SETFONT = 0x0030
        SendMessageW(
            edit,
            0x0030,
            GetStockObject(DEFAULT_GUI_FONT) as isize as usize,
            1,
        );
    }
    edit
}

unsafe fn composer_wnd_proc_msg(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    state: &mut State,
) -> Option<LRESULT> {
    match msg {
        WM_PAINT => {
            paint_composer(state);
            Some(0)
        }
        WM_ERASEBKGND => Some(1),
        WM_CTLCOLOREDIT => {
            let dc = wparam as *mut c_void;
            let dark = DARK_THEME.load(Ordering::Relaxed);
            if state.composer.bg_brush == 0 || state.composer.brush_dark != dark {
                if state.composer.bg_brush != 0 {
                    DeleteObject(state.composer.bg_brush as HGDIOBJ);
                }
                let bg = if dark { COLOR_DARK_BG } else { COLOR_LIGHT_BG };
                state.composer.bg_brush = CreateSolidBrush(bg) as isize;
                state.composer.brush_dark = dark;
            }
            let bg = if dark { COLOR_DARK_BG } else { COLOR_LIGHT_BG };
            let text = if dark {
                COLOR_DARK_TEXT
            } else {
                COLOR_LIGHT_TEXT
            };
            SetBkColor(dc, bg);
            SetTextColor(dc, text);
            Some(state.composer.bg_brush)
        }
        WM_MOUSEMOVE => {
            let x = (lparam as usize & 0xFFFF) as u16 as i16 as i32;
            let y = ((lparam as usize >> 16) & 0xFFFF) as u16 as i16 as i32;
            let hovered = in_send_button(x, y);
            if hovered != state.composer.button_hovered {
                state.composer.button_hovered = hovered;
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            Some(0)
        }
        WM_LBUTTONUP => {
            let x = (lparam as usize & 0xFFFF) as u16 as i16 as i32;
            let y = ((lparam as usize >> 16) & 0xFFFF) as u16 as i16 as i32;
            if in_send_button(x, y) {
                let text = read_window_text(state.composer.edit);
                if composer_send(state, &text) {
                    let empty = wide("");
                    SetWindowTextW(state.composer.edit, empty.as_ptr());
                }
            }
            Some(0)
        }
        WM_SETCURSOR => {
            let cursor_name = if state.composer.button_hovered {
                IDC_HAND
            } else {
                IDC_ARROW
            };
            SetCursor(LoadCursorW(ptr::null_mut(), cursor_name));
            Some(1)
        }
        WM_DESTROY => Some(0),
        _ => None,
    }
}

unsafe fn paint_composer(state: &mut State) {
    let hwnd = state.composer.hwnd;
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let dc = BeginPaint(hwnd, &mut ps);
    if dc.is_null() {
        EndPaint(hwnd, &ps);
        return;
    }
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rect);
    let dark = DARK_THEME.load(Ordering::Relaxed);
    let field = if dark { COLOR_DARK_BG } else { COLOR_LIGHT_BG };
    let background = CreateSolidBrush(field);
    if !background.is_null() {
        FillRect(dc, &rect, background);
        DeleteObject(background as HGDIOBJ);
    }

    // GDI+ gives the send button and the paper-plane glyph real
    // anti-aliasing; the rounded corners and 1 px border come from DWM (see
    // `apply_composer_dwm`), which keeps the child EDIT control working — a
    // layered `UpdateLayeredWindow` surface would hide the text input.
    if gdi_text::ensure_started() {
        let mut graphics: *mut GpGraphics = ptr::null_mut();
        if GdipCreateFromHDC(dc, &mut graphics) == 0 && !graphics.is_null() {
            GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);
            let mut accent = if dark { DARK_ACCENT } else { LIGHT_ACCENT };
            if state.composer.button_hovered {
                accent = lighten_argb(accent, 20);
            }
            let mut brush: *mut GpSolidFill = ptr::null_mut();
            if GdipCreateSolidFill(accent, &mut brush) == 0 && !brush.is_null() {
                let size = COMPOSER_SEND_SIZE as f32;
                let button_x = (COMPOSER_WIDTH - COMPOSER_SEND_SIZE - COMPOSER_SEND_INSET) as f32;
                let button_y = ((COMPOSER_HEIGHT - COMPOSER_SEND_SIZE) / 2) as f32;
                GdipFillEllipse(
                    graphics,
                    brush as *mut GpBrush,
                    button_x,
                    button_y,
                    size,
                    size,
                );
                let points = [
                    PointF {
                        X: button_x + size * 0.26,
                        Y: button_y + size * 0.52,
                    },
                    PointF {
                        X: button_x + size * 0.78,
                        Y: button_y + size * 0.26,
                    },
                    PointF {
                        X: button_x + size * 0.56,
                        Y: button_y + size * 0.78,
                    },
                    PointF {
                        X: button_x + size * 0.47,
                        Y: button_y + size * 0.56,
                    },
                ];
                GdipFillPolygon(
                    graphics,
                    brush as *mut GpBrush,
                    points.as_ptr(),
                    points.len() as i32,
                    FillModeWinding,
                );
                GdipDeleteBrush(brush as *mut GpBrush);
            }
            GdipDeleteGraphics(graphics);
        }
    }
    EndPaint(hwnd, &ps);
}

fn lighten_argb(color: u32, amount: u32) -> u32 {
    let a = color & 0xFF00_0000;
    let r = ((color >> 16) & 0xFF).saturating_add(amount).min(255);
    let g = ((color >> 8) & 0xFF).saturating_add(amount).min(255);
    let b = (color & 0xFF).saturating_add(amount).min(255);
    a | (r << 16) | (g << 8) | b
}

/// Rounds the composer like a modern system field and lets DWM draw the
/// anti-aliased 1 px frame; the colour follows the app theme.
unsafe fn apply_composer_dwm(hwnd: HWND, dark: bool) {
    if hwnd.is_null() {
        return;
    }
    let round: u32 = 2; // DWMWCP_ROUND
    let border: u32 = if dark {
        COLOR_DARK_BORDER
    } else {
        COLOR_LIGHT_BORDER
    };
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE as u32,
        &round as *const _ as *const c_void,
        std::mem::size_of::<u32>() as u32,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_BORDER_COLOR as u32,
        &border as *const _ as *const c_void,
        std::mem::size_of::<u32>() as u32,
    );
}

fn in_send_button(x: i32, y: i32) -> bool {
    let button_x = COMPOSER_WIDTH - COMPOSER_SEND_INSET - COMPOSER_SEND_SIZE;
    let button_y = (COMPOSER_HEIGHT - COMPOSER_SEND_SIZE) / 2;
    x >= button_x
        && x < button_x + COMPOSER_SEND_SIZE
        && y >= button_y
        && y < button_y + COMPOSER_SEND_SIZE
}

unsafe fn read_window_text(hwnd: HWND) -> String {
    let length = GetWindowTextLengthW(hwnd);
    if length <= 0 {
        return String::new();
    }
    let mut buffer = vec![0u16; (length + 1) as usize];
    let copied = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
    if copied <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..copied as usize])
}

/// Sends the composed text through the runtime; returns whether it was sent.
fn composer_send(state: &mut State, text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if let Ok(engine) = state.engine.lock() {
        let _ = engine.send(RuntimeCommand::SendConversation(trimmed.to_string()));
    }
    state.composer.draft.clear();
    log("composer: sent");
    true
}

/// Lazily installs the modern UI font (the stock `DEFAULT_GUI_FONT` bitmap
/// face was the main reason the old field looked out of place).
fn ensure_composer_font(state: &mut State) {
    if state.composer.font != 0 || state.composer.edit.is_null() {
        return;
    }
    unsafe {
        let face = wide("Microsoft YaHei UI");
        let font = CreateFontW(
            -14,
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH as u32) | (FF_DONTCARE as u32),
            face.as_ptr(),
        );
        if !font.is_null() {
            state.composer.font = font as isize;
            SendMessageW(state.composer.edit, 0x0030, font as usize, 1);
        }
    }
}

/// Brings the composer forward and focuses its edit control. The pet window
/// never activates, so a plain `SetForegroundWindow` can be rejected while
/// another app owns the foreground; attaching to that thread's input queue is
/// the documented workaround and injects no synthetic input.
unsafe fn focus_composer(hwnd: HWND, edit: HWND) {
    let foreground = GetForegroundWindow();
    let target_thread = if foreground.is_null() || foreground == hwnd {
        0
    } else {
        GetWindowThreadProcessId(foreground, ptr::null_mut())
    };
    let current = GetCurrentThreadId();
    let attached = target_thread != 0
        && target_thread != current
        && AttachThreadInput(current, target_thread, 1) != 0;
    SetForegroundWindow(hwnd);
    SetFocus(edit);
    if attached {
        AttachThreadInput(current, target_thread, 0);
    }
}

fn open_composer(state: &mut State) {
    if state.composer.hwnd.is_null() || state.composer.edit.is_null() {
        return;
    }
    ensure_composer_font(state);
    state.composer.open = true;
    state.composer.focus_deadline = Some(Instant::now() + std::time::Duration::from_millis(1500));
    unsafe {
        let mut pet_rect: RECT = std::mem::zeroed();
        if GetWindowRect(state.pet_hwnd, &mut pet_rect) != 0 {
            let work = work_area_for_rect(pet_rect);
            state.composer_side = Some(choose_side(
                pet_rect,
                work,
                COMPOSER_HEIGHT,
                state.last_side,
            ));
            state.last_side = state.composer_side;
        }
    }
    unsafe {
        let draft = wide(&state.composer.draft);
        SetWindowTextW(state.composer.edit, draft.as_ptr());
        update_composer(state);
        ShowWindow(state.composer.hwnd, SW_SHOW);
        focus_composer(state.composer.hwnd, state.composer.edit);
    }
    log("composer: opened");
}

/// Keeps the composer under the pet and retries taking focus for 1.5 s.
fn update_composer(state: &mut State) {
    if state.composer.hwnd.is_null() || !state.composer.open {
        return;
    }
    let dark = DARK_THEME.load(Ordering::Relaxed);
    if state.composer.dwm_dark != Some(dark) {
        unsafe {
            apply_composer_dwm(state.composer.hwnd, dark);
        }
        state.composer.dwm_dark = Some(dark);
    }
    unsafe {
        let mut pet_rect: RECT = std::mem::zeroed();
        if GetWindowRect(state.pet_hwnd, &mut pet_rect) == 0 {
            return;
        }
        let work = work_area_for_rect(pet_rect);
        let side = match state.composer_side {
            Some(side) => side,
            None => {
                let side = choose_side(pet_rect, work, COMPOSER_HEIGHT, state.last_side);
                state.composer_side = Some(side);
                side
            }
        };
        let (x, y) = position_panel(pet_rect, side, COMPOSER_WIDTH, COMPOSER_HEIGHT);
        if state.composer.position != (x, y) {
            state.composer.position = (x, y);
            SetWindowPos(
                state.composer.hwnd,
                ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }

        if let Some(deadline) = state.composer.focus_deadline {
            if Instant::now() >= deadline {
                state.composer.focus_deadline = None;
            } else if GetForegroundWindow() != state.composer.hwnd {
                focus_composer(state.composer.hwnd, state.composer.edit);
            } else {
                state.composer.focus_deadline = None;
                SetFocus(state.composer.edit);
            }
        }
    }
}

unsafe fn work_area_for_rect(rect: RECT) -> RECT {
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    let monitor = MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST);
    if monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
        return RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
    }
    info.rcWork
}

/// Port of `OverlayLayout.ChooseSide`: below by default, left/right when the
/// taskbar leaves no room, with hysteresis so the side does not flip-flop.
fn choose_side(
    pet: RECT,
    work: RECT,
    panel_height: i32,
    current: Option<OverlaySide>,
) -> OverlaySide {
    let below = work.bottom - pet.bottom - GAP - EDGE_MARGIN;
    let right = work.right - pet.right - GAP - EDGE_MARGIN;
    let left = pet.left - work.left - GAP - EDGE_MARGIN;
    let bottom_fits = below >= panel_height;

    match current {
        Some(OverlaySide::Bottom) => {
            if bottom_fits {
                OverlaySide::Bottom
            } else if right >= left {
                OverlaySide::Right
            } else {
                OverlaySide::Left
            }
        }
        Some(side @ (OverlaySide::Left | OverlaySide::Right)) => {
            if below >= panel_height + 16 {
                return OverlaySide::Bottom;
            }
            let (current_space, other_space) = if side == OverlaySide::Right {
                (right, left)
            } else {
                (left, right)
            };
            if current_space >= COMPOSER_MIN_WIDTH || current_space >= other_space {
                side
            } else if side == OverlaySide::Right {
                OverlaySide::Left
            } else {
                OverlaySide::Right
            }
        }
        None => {
            if bottom_fits {
                OverlaySide::Bottom
            } else if right >= left {
                OverlaySide::Right
            } else {
                OverlaySide::Left
            }
        }
    }
}

/// Port of `OverlayLayout.PositionPanel` for the composer.
unsafe fn position_panel(pet: RECT, side: OverlaySide, width: i32, height: i32) -> (i32, i32) {
    let x = match side {
        OverlaySide::Left => pet.left - width - GAP,
        OverlaySide::Right => pet.right + GAP,
        OverlaySide::Bottom => pet.left + ((pet.right - pet.left) - width) / 2,
    };
    let y = match side {
        OverlaySide::Bottom => pet.bottom + GAP,
        _ => pet.bottom - height + 6,
    };
    let desired = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    };
    let clamped = clamp_to_work_area(desired);
    (clamped.left, clamped.top)
}

unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_KEYDOWN {
        if wparam == VK_ESCAPE as usize {
            let text = read_window_text(hwnd);
            let _ = with_state(
                |state| {
                    state.composer.draft = text;
                    close_composer(state);
                },
                (),
            );
            return 0;
        }
        if wparam == VK_RETURN as usize {
            let shift_down = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
            if !shift_down {
                let text = read_window_text(hwnd);
                let sent = with_state(|state| composer_send(state, &text), false);
                if sent {
                    let empty = wide("");
                    let result = SetWindowTextW(hwnd, empty.as_ptr());
                    let remaining = read_window_text(hwnd);
                    log(&format!(
                        "composer: cleared result={} remaining='{remaining}'",
                        result
                    ));
                }
                return 0;
            }
        }
    }

    let old = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if old != 0 {
        let previous: Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT> =
            std::mem::transmute(old);
        return CallWindowProcW(previous, hwnd, msg, wparam, lparam);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Posts `WM_OPEN_COMPOSER` to the pet window (test hook / future entries).
pub fn request_open_composer() {
    let hwnd = PET_HWND.load(Ordering::Relaxed);
    if hwnd != 0 {
        unsafe {
            PostMessageW(hwnd as HWND, WM_OPEN_COMPOSER, 0, 0);
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        hover_should_jump, next_drag_direction, wheel_travel_action, PetState, WheelAction,
        WheelTravel,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn drag_direction_uses_the_latest_step_with_a_flip_filter() {
        let mut accumulator = 0;
        assert_eq!(
            next_drag_direction(None, 5, &mut accumulator),
            Some(PetState::RunningRight)
        );
        assert_eq!(
            next_drag_direction(Some(PetState::RunningRight), 4, &mut accumulator),
            None
        );
        assert_eq!(accumulator, 0);
        assert_eq!(
            next_drag_direction(Some(PetState::RunningRight), -2, &mut accumulator),
            None
        );
        assert_eq!(accumulator, 2);
        assert_eq!(
            next_drag_direction(Some(PetState::RunningRight), -1, &mut accumulator),
            Some(PetState::RunningLeft)
        );
        assert_eq!(accumulator, 0);
        assert_eq!(
            next_drag_direction(Some(PetState::RunningLeft), 0, &mut accumulator),
            None
        );
    }

    #[test]
    fn wheel_travel_needs_three_notches_before_toggling() {
        let mut travel = WheelTravel::default();
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        assert_eq!(wheel_travel_action(&mut travel, -120, t0), None);
        assert_eq!(wheel_travel_action(&mut travel, -120, t0 + ms(40)), None);
        assert_eq!(
            wheel_travel_action(&mut travel, -120, t0 + ms(80)),
            Some(WheelAction::Open)
        );
        // the accumulator resets after a trigger, so the opposite direction
        // still needs its own full travel
        assert_eq!(wheel_travel_action(&mut travel, 120, t0 + ms(120)), None);
        assert_eq!(wheel_travel_action(&mut travel, 120, t0 + ms(160)), None);
        assert_eq!(
            wheel_travel_action(&mut travel, 120, t0 + ms(200)),
            Some(WheelAction::Close)
        );
    }

    #[test]
    fn wheel_travel_resets_on_pause_and_direction_flip() {
        let mut travel = WheelTravel::default();
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        assert_eq!(wheel_travel_action(&mut travel, -120, t0), None);
        assert_eq!(wheel_travel_action(&mut travel, -120, t0 + ms(50)), None);
        // a pause longer than the window drops the two notches above
        assert_eq!(wheel_travel_action(&mut travel, -120, t0 + ms(900)), None);
        // a flip discards the down travel and starts counting up
        assert_eq!(wheel_travel_action(&mut travel, 120, t0 + ms(950)), None);
        assert_eq!(wheel_travel_action(&mut travel, 120, t0 + ms(1000)), None);
        assert_eq!(
            wheel_travel_action(&mut travel, 120, t0 + ms(1050)),
            Some(WheelAction::Close)
        );
    }

    #[test]
    fn wheel_travel_accepts_high_resolution_deltas() {
        let mut travel = WheelTravel::default();
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        for step in 0..11 {
            assert_eq!(
                wheel_travel_action(&mut travel, -30, t0 + ms(step * 10)),
                None
            );
        }
        assert_eq!(
            wheel_travel_action(&mut travel, -30, t0 + ms(110)),
            Some(WheelAction::Open)
        );
    }

    #[test]
    fn hover_jump_fires_once_on_enter_and_respects_busy_states() {
        assert!(hover_should_jump(false, true, false, false, false, true));
        assert!(!hover_should_jump(true, true, false, false, false, true));
        assert!(!hover_should_jump(false, false, false, false, false, true));
        assert!(!hover_should_jump(false, true, true, false, false, true));
        assert!(!hover_should_jump(false, true, false, true, false, true));
        assert!(!hover_should_jump(false, true, false, false, true, true));
        assert!(!hover_should_jump(false, true, false, false, false, false));
    }
}
