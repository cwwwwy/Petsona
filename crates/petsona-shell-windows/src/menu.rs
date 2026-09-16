//! Dedicated Win32 popup-menu thread.
//!
//! Windows' `TrackPopupMenu` runs a nested modal loop. Keeping it on this
//! thread lets the eframe thread continue animating the pet and processing
//! state protocol events while the native menu is open.
//!
//! The owner window is a real (hidden) top-level window on purpose:
//!
//! - a message-only window (`HWND_MESSAGE`) can never become the foreground
//!   window, and without foreground the menu never receives Escape or the
//!   click that should dismiss it;
//! - `WS_EX_NOACTIVATE` would block `SetForegroundWindow` for the same reason.
//!
//! Focus is handed back to the previously active window as soon as the menu
//! closes, so opening a menu does not steal the user's editor focus (B7).

use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use egui::Context as EguiContext;
use petsona_app::platform::{MenuCommand, PlatformMenu};

use crate::platform::clamp_point_to_work_area;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetForegroundWindow, GetMessageW, ModifyMenuW, PostMessageW,
    PostThreadMessageW, RegisterClassW, SetForegroundWindow, TrackPopupMenuEx, TranslateMessage,
    UnregisterClassW, MA_NOACTIVATE, MF_BYCOMMAND, MF_STRING, MSG, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_WORKAREA, WM_APP, WM_CANCELMODE, WM_MOUSEACTIVATE, WM_NULL, WM_QUIT, WNDCLASSW,
    WS_EX_TOOLWINDOW, WS_POPUP,
};

const COMMAND_OPEN_SETTINGS: u32 = 1;
const COMMAND_CHANGE_PET: u32 = 2;
const COMMAND_TOGGLE_PET: u32 = 3;
const COMMAND_QUIT: u32 = 4;

const CLASS_NAME: &str = "Petsona.NativeMenu";

/// Posted to the owner window to tell the menu thread that a request is waiting
/// in the channel. The thread has to run a message loop anyway: it owns a
/// window, and any `SendMessage`-based call from another thread (for example
/// `GetWindowTextW` while enumerating windows) would block forever otherwise.
const WM_PETSONA_SHOW: u32 = WM_APP + 1;

enum Request {
    Show { x: i32, y: i32, pet_visible: bool },
    Shutdown,
}

/// Win32 popup menu living on its own thread.
pub struct WindowsMenu {
    requests: Sender<Request>,
    commands: Receiver<u32>,
    thread_id: u32,
    owner: isize,
    /// True while `TrackPopupMenuEx` is running, so a second click can dismiss
    /// the open menu instead of queueing another one behind it.
    open: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl WindowsMenu {
    pub fn start(ctx: &EguiContext) -> Result<Self> {
        let (request_tx, request_rx) = mpsc::channel();
        let (command_tx, command_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let wake = ctx.clone();
        let open = Arc::new(AtomicBool::new(false));
        let thread_open = Arc::clone(&open);

        let handle = thread::Builder::new()
            .name("petsona-win32-menu".to_string())
            .spawn(move || run_menu_thread(request_rx, command_tx, ready_tx, wake, thread_open))
            .context("cannot start the Win32 menu thread")?;

        let (thread_id, owner) = ready_rx
            .recv_timeout(Duration::from_secs(2))
            .context("Win32 menu thread did not become ready")?
            .map_err(|error| anyhow!(error))?;

        Ok(Self {
            requests: request_tx,
            commands: command_rx,
            thread_id,
            owner,
            open,
            handle: Some(handle),
        })
    }
}

impl PlatformMenu for WindowsMenu {
    fn show(&self, x: f64, y: f64, pet_visible: bool) {
        let _ = self.requests.send(Request::Show {
            x: x.round() as i32,
            y: y.round() as i32,
            pet_visible,
        });
        let owner = self.owner as HWND;
        if !owner.is_null() {
            unsafe {
                let _ = PostMessageW(owner, WM_PETSONA_SHOW, 0, 0);
            }
        }
        // Close the menu that is currently open so the queued request is the
        // one that gets shown: repeated clicks move the menu instead of
        // stacking one menu per click.
        if self.open.load(Ordering::SeqCst) && !owner.is_null() {
            unsafe {
                let _ = PostMessageW(owner, WM_CANCELMODE, 0, 0);
            }
        }
    }

    fn dismiss(&self) {
        let owner = self.owner as HWND;
        if !owner.is_null() {
            unsafe {
                let _ = PostMessageW(owner, WM_CANCELMODE, 0, 0);
            }
        }
    }

    fn poll(&self) -> Vec<MenuCommand> {
        self.commands
            .try_iter()
            .filter_map(|command| match command {
                COMMAND_OPEN_SETTINGS => Some(MenuCommand::OpenSettings),
                COMMAND_CHANGE_PET => Some(MenuCommand::ChangePet),
                COMMAND_TOGGLE_PET => Some(MenuCommand::TogglePet),
                COMMAND_QUIT => Some(MenuCommand::Quit),
                _ => None,
            })
            .collect()
    }
}

impl Drop for WindowsMenu {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Shutdown);
        let owner = self.owner as HWND;
        if !owner.is_null() {
            unsafe {
                let _ = PostMessageW(owner, WM_CANCELMODE, 0, 0);
            }
        }
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run_menu_thread(
    requests: Receiver<Request>,
    commands: Sender<u32>,
    ready: mpsc::SyncSender<Result<(u32, isize)>>,
    wake: EguiContext,
    open: Arc<AtomicBool>,
) {
    let class_name = wide(CLASS_NAME);
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    let class = WNDCLASSW {
        lpfnWndProc: Some(menu_window_proc),
        hInstance: instance,
        lpszClassName: class_name.as_ptr(),
        ..Default::default()
    };
    unsafe {
        let _ = RegisterClassW(&class);
    }

    // A hidden top-level window: it must be able to take the foreground so the
    // menu receives Escape and dismiss-on-outside-click, which a message-only
    // window cannot do.
    let owner = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        )
    };
    if owner.is_null() {
        let _ = ready.send(Err(anyhow!("cannot create the Win32 menu owner window")));
        return;
    }

    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        unsafe {
            let _ = DestroyWindow(owner);
            let _ = UnregisterClassW(class_name.as_ptr(), instance);
        }
        let _ = ready.send(Err(anyhow!("cannot create the Win32 popup menu")));
        return;
    }

    if !append_item(menu, COMMAND_OPEN_SETTINGS as usize, "打开设置")
        || !append_item(menu, COMMAND_CHANGE_PET as usize, "更换宠物")
        || !append_item(menu, COMMAND_TOGGLE_PET as usize, "隐藏宠物")
        || !append_item(menu, COMMAND_QUIT as usize, "退出")
    {
        unsafe {
            let _ = DestroyMenu(menu);
            let _ = DestroyWindow(owner);
            let _ = UnregisterClassW(class_name.as_ptr(), instance);
        }
        let _ = ready.send(Err(anyhow!("cannot populate the Win32 popup menu")));
        return;
    }

    let thread_id = unsafe { GetCurrentThreadId() };
    if ready.send(Ok((thread_id, owner as isize))).is_err() {
        unsafe {
            let _ = DestroyMenu(menu);
            let _ = DestroyWindow(owner);
            let _ = UnregisterClassW(class_name.as_ptr(), instance);
        }
        return;
    }

    let mut message = MSG::default();
    'outer: while unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) } > 0 {
        if message.message != WM_PETSONA_SHOW {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            continue;
        }

        // One wake-up can cover several clicks, and a click that arrived while
        // a menu was already open is queued behind it. Showing the newest
        // request until the queue is empty is what makes repeated clicks move
        // the menu instead of opening one menu per click.
        loop {
            let mut newest: Option<(i32, i32, bool)> = None;
            let mut shutdown = false;
            loop {
                match requests.try_recv() {
                    Ok(Request::Show { x, y, pet_visible }) => newest = Some((x, y, pet_visible)),
                    Ok(Request::Shutdown) => {
                        shutdown = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => break 'outer,
                }
            }
            if shutdown {
                break 'outer;
            }
            let Some((x, y, pet_visible)) = newest else {
                break;
            };
            // `TPM_WORKAREA` keeps the menu rectangle on screen, but the
            // *anchor* is where the user clicked; clamp it into the nearest
            // monitor's work area so a pet parked against a screen edge still
            // opens the menu on the monitor it is on.
            let (x, y) = clamp_point_to_work_area(x, y, 8);

            let toggle = if pet_visible {
                "隐藏宠物"
            } else {
                "显示宠物"
            };
            let toggle_text = wide(toggle);
            let command = unsafe {
                let _ = ModifyMenuW(
                    menu,
                    COMMAND_TOGGLE_PET,
                    MF_BYCOMMAND | MF_STRING,
                    COMMAND_TOGGLE_PET as usize,
                    toggle_text.as_ptr(),
                );
                // Hand the foreground to our own hidden owner so the menu is
                // dismissible (Escape, click outside) and keyboard navigable.
                // The window that lost the foreground gets it back below.
                let previous = GetForegroundWindow();
                SetForegroundWindow(owner);
                open.store(true, Ordering::SeqCst);
                let command = TrackPopupMenuEx(
                    menu,
                    TPM_RETURNCMD | TPM_NONOTIFY | TPM_WORKAREA,
                    x,
                    y,
                    owner,
                    ptr::null(),
                );
                open.store(false, Ordering::SeqCst);
                let _ = PostMessageW(owner, WM_NULL, 0, 0);
                if !previous.is_null() && previous != owner {
                    SetForegroundWindow(previous);
                }
                command
            };

            if command != 0 {
                let _ = commands.send(command as u32);
                wake.request_repaint();
            }
        }
    }

    unsafe {
        let _ = DestroyMenu(menu);
        let _ = DestroyWindow(owner);
        let _ = UnregisterClassW(class_name.as_ptr(), instance);
    }
}
fn append_item(
    menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    id: usize,
    text: &str,
) -> bool {
    let text = wide(text);
    unsafe { AppendMenuW(menu, MF_STRING, id, text.as_ptr()) != 0 }
}

fn wide(text: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt as _;
    std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// The owner window is never shown and receives no input of its own; it exists
/// only to own the popup menu, so the default handling is enough.
unsafe extern "system" fn menu_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as LRESULT;
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}
