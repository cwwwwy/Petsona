use std::cell::RefCell;
use std::collections::HashMap;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, SetWindowLongPtrW, GWLP_WNDPROC, GWL_EXSTYLE, GWL_STYLE,
    MA_NOACTIVATE, STYLESTRUCT, WM_MOUSEACTIVATE, WM_NCDESTROY, WM_STYLECHANGING, WNDPROC,
    WS_BORDER, WS_CAPTION, WS_DLGFRAME, WS_EX_NOACTIVATE, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP,
    WS_SYSMENU,
};

thread_local! {
    static PREVIOUS: RefCell<HashMap<isize, isize>> = RefCell::new(HashMap::new());
}

pub(crate) fn install(hwnd: HWND) {
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
            let frame =
                WS_CAPTION | WS_BORDER | WS_DLGFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
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
