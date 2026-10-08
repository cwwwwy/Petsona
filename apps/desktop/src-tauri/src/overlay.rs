//! M0 spike: native layered pet window on a dedicated thread with its own
//! message pump, independent of Tauri's window management.
//!
//! Minimal port of the deleted C# frontend (git 6bca241):
//! - `WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE` popup window
//! - `UpdateLayeredWindow` with a top-down premultiplied BGRA DIB
//! - 50 ms hit-test poll toggling `WS_EX_TRANSPARENT`
//! - `WM_NCHITTEST` returns `HTTRANSPARENT` outside opaque pixels
//! - drag with a 4 px threshold using `SWP_NOACTIVATE` moves
//! - arrow cursor installed on the window class

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HGDIOBJ,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, GetMessageW,
    GetWindowLongPtrW, GetWindowRect, KillTimer, LoadCursorW, PostQuitMessage, RegisterClassExW,
    SetCursor, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage,
    UpdateLayeredWindow, GWL_EXSTYLE, IDC_ARROW, MSG, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SWP_NOSIZE, SWP_NOZORDER, ULW_ALPHA, WM_DESTROY, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_NCHITTEST, WM_SETCURSOR, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::logging::log;

const CLASS_NAME: &str = "PetsonaPetWindow";
const ANIM_TIMER: usize = 1;
const HIT_TIMER: usize = 2;
const ANIM_INTERVAL_MS: u32 = 120;
const HIT_INTERVAL_MS: u32 = 50;
const ALPHA_THRESHOLD: u8 = 13;
const CELL: u32 = 64;
const IDLE_FRAMES: u32 = 2;
const SCALE: u32 = 3;
const WINDOW_W: i32 = (CELL * SCALE) as i32;
const WINDOW_H: i32 = (CELL * SCALE) as i32;
const DRAG_THRESHOLD: i32 = 4;

const HIT_CLIENT: LRESULT = 1;
const HIT_TRANSPARENT: LRESULT = -1;

struct Frame {
    width: i32,
    height: i32,
    /// Premultiplied BGRA, top-down, ready for `UpdateLayeredWindow`.
    pixels: Vec<u8>,
    mask: Vec<bool>,
}

struct State {
    frames: Vec<Frame>,
    idle_union: Vec<bool>,
    index: usize,
    shown: bool,
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

/// Spawns the overlay thread. Idempotent per process for the spike.
pub fn spawn() {
    let spawned = std::thread::Builder::new()
        .name("petsona-overlay".into())
        .spawn(|| unsafe { thread_main() });
    if let Err(err) = spawned {
        log(&format!("overlay: thread spawn failed: {err}"));
    }
}

unsafe fn thread_main() {
    let frames = match load_frames() {
        Ok(frames) => frames,
        Err(err) => {
            log(&format!("overlay: frame load failed: {err}"));
            return;
        }
    };

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

    let idle_union = union_masks(&frames);
    let mut state = Box::new(State {
        frames,
        idle_union,
        index: 0,
        shown: false,
        dragging: false,
        moved: false,
        drag_cursor: (0, 0),
        drag_offset: (0, 0),
        transparent: None,
    });

    present(hwnd, &state.frames[0]);
    state.shown = true;
    STATE.with(|cell| *cell.borrow_mut() = Some(state));

    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    SetTimer(hwnd, ANIM_TIMER, ANIM_INTERVAL_MS, None);
    SetTimer(hwnd, HIT_TIMER, HIT_INTERVAL_MS, None);
    log("overlay: pet window shown");

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
        420,
        260,
        WINDOW_W,
        WINDOW_H,
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
    let dib = CreateDIBSection(
        mem_dc,
        &info,
        DIB_RGB_COLORS,
        &mut bits,
        ptr::null_mut(),
        0,
    );
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
                if wparam == ANIM_TIMER {
                    state.index = (state.index + 1) % state.frames.len();
                    present(hwnd, &state.frames[state.index]);
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
                KillTimer(hwnd, ANIM_TIMER);
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
        "pet: mouse down cursor=({},{}) rect=({},{}) offset=({},{})",
        cursor.x, cursor.y, rect.left, rect.top, state.drag_offset.0, state.drag_offset.1
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
        log(&format!(
            "pet: drag started cursor=({},{}) start=({},{})",
            cursor.x, cursor.y, state.drag_cursor.0, state.drag_cursor.1
        ));
    }
    SetWindowPos(
        hwnd,
        ptr::null_mut(),
        cursor.x - state.drag_offset.0,
        cursor.y - state.drag_offset.1,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

unsafe fn on_lbutton_up(_hwnd: HWND, state: &mut State) {
    if !state.dragging {
        return;
    }
    ReleaseCapture();
    state.dragging = false;
    if state.moved {
        log("pet: drag ended");
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

unsafe fn update_pass_through(hwnd: HWND, state: &mut State) {
    if !state.shown {
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
    let local_x = cursor.x - rect.left;
    let local_y = cursor.y - rect.top;
    let hit = hit(state, local_x, local_y);
    let want_transparent = !hit;
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
    let Some(frame) = state.frames.get(state.index) else {
        return false;
    };
    if x < 0 || y < 0 || x >= frame.width || y >= frame.height {
        return false;
    }
    let index = (y as usize) * (frame.width as usize) + (x as usize);
    state.idle_union.get(index).copied().unwrap_or(false)
        || frame.mask.get(index).copied().unwrap_or(false)
}

fn load_frames() -> Result<Vec<Frame>, String> {
    const ATLAS_PNG: &[u8] =
        include_bytes!("../../../../crates/petsona-core/testdata/v2-test-pet/spritesheet.png");
    let atlas = image::load_from_memory(ATLAS_PNG)
        .map_err(|err| err.to_string())?
        .to_rgba8();

    let mut frames = Vec::with_capacity(IDLE_FRAMES as usize);
    for index in 0..IDLE_FRAMES {
        let cell = image::imageops::crop_imm(&atlas, index * CELL, 0, CELL, CELL).to_image();
        let scaled = image::imageops::resize(
            &cell,
            CELL * SCALE,
            CELL * SCALE,
            image::imageops::FilterType::Nearest,
        );
        let (width, height) = scaled.dimensions();
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
        frames.push(Frame {
            width: width as i32,
            height: height as i32,
            pixels,
            mask,
        });
    }

    if frames.is_empty() {
        return Err("no frames decoded".into());
    }
    Ok(frames)
}

fn union_masks(frames: &[Frame]) -> Vec<bool> {
    let Some(first) = frames.first() else {
        return Vec::new();
    };
    let mut union = first.mask.clone();
    for frame in &frames[1..] {
        for (index, value) in frame.mask.iter().enumerate() {
            union[index] |= *value;
        }
    }
    union
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
