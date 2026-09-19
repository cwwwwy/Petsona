use std::cell::RefCell;
use std::collections::HashMap;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, SetWindowLongPtrW, SetWindowPos, GWLP_WNDPROC, GWL_EXSTYLE,
    GWL_STYLE, MA_NOACTIVATE, STYLESTRUCT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, WM_MOUSEACTIVATE, WM_NCCALCSIZE, WM_NCDESTROY, WM_NCPAINT,
    WM_STYLECHANGING, WNDPROC, WS_BORDER, WS_CAPTION, WS_DLGFRAME, WS_EX_NOACTIVATE,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
};

thread_local! {
    static PREVIOUS: RefCell<HashMap<isize, (isize, bool)>> = RefCell::new(HashMap::new());
}

/// Frameless windows plus "never take focus" (pet, bubble, shadow, menu).
pub(crate) fn install(hwnd: HWND) {
    install_with(hwnd, false);
}

/// Frameless geometry only: the composer must stay activatable.
pub(crate) fn install_frameless(hwnd: HWND) {
    install_with(hwnd, true);
}

fn install_with(hwnd: HWND, frameless_only: bool) {
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
    if previous == 0 {
        return;
    }
    PREVIOUS.with(|previous_procs| {
        previous_procs
            .borrow_mut()
            .insert(key, (previous, frameless_only));
    });
    // Win32 computes the non-client area when the window is created; a
    // viewport window keeps a 1px strip at the top even after the frame styles
    // are gone. That strip paints a white line on transparent overlays and
    // clips the bottom row of the content. Force a recalculation now that our
    // window procedure answers `WM_NCCALCSIZE` with "client == whole window".
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

unsafe extern "system" fn no_activate_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Let the client area cover the entire window: no caption strip, no white
    // top line, no clipped bottom row.
    if message == WM_NCCALCSIZE && wparam != 0 {
        return 0;
    }
    if message == WM_NCPAINT {
        return 0;
    }
    if message == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as LRESULT;
    }
    let frameless_only = PREVIOUS.with(|previous_procs| {
        previous_procs
            .borrow()
            .get(&(hwnd as isize))
            .is_some_and(|(_, frameless_only)| *frameless_only)
    });
    if message == WM_STYLECHANGING && lparam != 0 {
        let styles = unsafe { &mut *(lparam as *mut STYLESTRUCT) };
        let index = wparam as i32;
        if index == GWL_STYLE {
            // winit rewrites the whole style whenever one of its window flags
            // changes (mouse passthrough, visible, ...); it puts the classic
            // caption/frame bits back, which flashed a native border on the
            // otherwise frameless overlays. Veto those bits here so the
            // decorated style never reaches the window.
            let frame = WS_CAPTION
                | WS_BORDER
                | WS_DLGFRAME
                | WS_SYSMENU
                | WS_MINIMIZEBOX
                | WS_MAXIMIZEBOX
                | WS_THICKFRAME;
            styles.styleNew = (styles.styleNew & !frame) | WS_POPUP;
        } else if index == GWL_EXSTYLE && !frameless_only {
            styles.styleNew |= WS_EX_NOACTIVATE;
        }
    }

    let key = hwnd as isize;
    let previous = PREVIOUS.with(|previous_procs| {
        previous_procs
            .borrow()
            .get(&key)
            .map(|(previous, _)| *previous)
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
