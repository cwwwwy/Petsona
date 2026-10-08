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
use std::sync::{Arc, Mutex, OnceLock};

use petsona_runtime::commands::RuntimeCommand;
use petsona_runtime::engine::RuntimeEngine;
use petsona_runtime::snapshot::RuntimeTextField;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW,
    MonitorFromPoint, MonitorFromRect, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    MONITOR_DEFAULTTOPRIMARY,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, GetMessageW,
    GetWindowLongPtrW, GetWindowRect, KillTimer, LoadCursorW, PostQuitMessage, RegisterClassExW,
    SetCursor, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage,
    UpdateLayeredWindow, GWL_EXSTYLE, IDC_ARROW, MSG, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_DESTROY, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_NCHITTEST, WM_SETCURSOR, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::logging::log;

const CLASS_NAME: &str = "PetsonaPetWindow";
const VISUAL_TIMER: usize = 1;
const HIT_TIMER: usize = 2;
const VISUAL_INTERVAL_MS: u32 = 33;
const HIT_INTERVAL_MS: u32 = 50;
const ALPHA_THRESHOLD: u8 = 13;
const DRAG_THRESHOLD: i32 = 4;
const FRAME_CACHE_LIMIT: usize = 256;
const DEFAULT_MARGIN: i32 = 24;

const HIT_CLIENT: LRESULT = 1;
const HIT_TRANSPARENT: LRESULT = -1;

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

struct State {
    engine: Arc<Mutex<RuntimeEngine>>,
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
    transparent: Option<bool>,
}

thread_local! {
    static STATE: RefCell<Option<Box<State>>> = const { RefCell::new(None) };
}

static CLASS_REGISTERED: OnceLock<bool> = OnceLock::new();

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
    if !register_class() {
        log("overlay: RegisterClassExW failed");
        return;
    }

    let hwnd = create_window();
    if hwnd.is_null() {
        log("overlay: CreateWindowExW failed");
        return;
    }
    apply_dwm_attributes(hwnd);

    let state = Box::new(State {
        engine,
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
        transparent: None,
    });
    STATE.with(|cell| *cell.borrow_mut() = Some(state));

    SetTimer(hwnd, VISUAL_TIMER, VISUAL_INTERVAL_MS, None);
    SetTimer(hwnd, HIT_TIMER, HIT_INTERVAL_MS, None);
    log("overlay: pet window created (hidden until the runtime is ready)");

    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    log("overlay: message loop ended");
}

unsafe fn register_class() -> bool {
    *CLASS_REGISTERED.get_or_init(|| {
        let class_w = wide(CLASS_NAME);
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

unsafe fn create_window() -> HWND {
    let class_w = wide(CLASS_NAME);
    let title_w = wide("Petsona");
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
        match msg {
            WM_TIMER => {
                if wparam == VISUAL_TIMER {
                    refresh(hwnd, state);
                } else if wparam == HIT_TIMER {
                    update_pass_through(hwnd, state);
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
            WM_SETCURSOR => {
                SetCursor(LoadCursorW(ptr::null_mut(), IDC_ARROW));
                Some(1)
            }
            WM_DESTROY => {
                KillTimer(hwnd, VISUAL_TIMER);
                KillTimer(hwnd, HIT_TIMER);
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
    let (snapshot, atlas_path) = {
        let Ok(engine) = state.engine.lock() else {
            return;
        };
        (engine.snapshot(), engine.text(RuntimeTextField::AtlasPath))
    };

    let want_visible =
        snapshot.ready && snapshot.has_pet && snapshot.pet_visible && !atlas_path.is_empty();

    if want_visible != state.visible {
        if want_visible {
            if !render_frame(hwnd, state, &snapshot, &atlas_path) {
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
        return;
    }

    if state.visible {
        render_frame(hwnd, state, &snapshot, &atlas_path);
    }
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
        present(hwnd, frame);
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

unsafe fn present(hwnd: HWND, frame: &Frame) {
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
        biWidth: frame.width,
        biHeight: -frame.height, // top-down
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
    ptr::copy_nonoverlapping(frame.pixels.as_ptr(), bits as *mut u8, frame.pixels.len());

    let size = SIZE {
        cx: frame.width,
        cy: frame.height,
    };
    let src = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: 0, // AC_SRC_OVER
        BlendFlags: 0,
        SourceConstantAlpha: 255,
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

unsafe fn on_lbutton_down(hwnd: HWND, state: &mut State) {
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
    state.drag_offset = (cursor.x - rect.left, cursor.y - rect.top);
    SetCapture(hwnd);
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
    } else {
        log("pet: clicked");
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

/// 50 ms poll toggling `WS_EX_TRANSPARENT` so transparent pixels pass clicks
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

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
