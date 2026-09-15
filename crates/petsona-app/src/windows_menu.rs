//! Dedicated Win32 popup-menu thread.
//!
//! Windows' TrackPopupMenu runs a nested modal loop. Keeping it on this
//! thread lets the eframe thread continue animating the pet and processing
//! state protocol events while the native menu is open.

use std::ptr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use egui::Context as EguiContext;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    ModifyMenuW, PostMessageW, PostThreadMessageW, RegisterClassW, TrackPopupMenuEx,
    UnregisterClassW, HWND_MESSAGE, MA_NOACTIVATE, MF_BYCOMMAND, MF_STRING, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_WORKAREA, WM_CANCELMODE, WM_MOUSEACTIVATE, WM_QUIT, WNDCLASSW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};

pub const COMMAND_OPEN_SETTINGS: u32 = 1;
pub const COMMAND_CHANGE_PET: u32 = 2;
pub const COMMAND_TOGGLE_PET: u32 = 3;
pub const COMMAND_QUIT: u32 = 4;

const CLASS_NAME: &str = "Petsona.NativeMenu";

enum Request {
    Show { x: i32, y: i32, pet_visible: bool },
    Shutdown,
}

pub struct WindowsMenu {
    requests: Sender<Request>,
    commands: Receiver<u32>,
    thread_id: u32,
    owner: isize,
    handle: Option<JoinHandle<()>>,
}

impl WindowsMenu {
    pub fn start(ctx: &EguiContext) -> Result<Self> {
        let (request_tx, request_rx) = mpsc::channel();
        let (command_tx, command_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let wake = ctx.clone();

        let handle = thread::Builder::new()
            .name("petsona-win32-menu".to_string())
            .spawn(move || run_menu_thread(request_rx, command_tx, ready_tx, wake))
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
            handle: Some(handle),
        })
    }

    pub fn show(&self, x: i32, y: i32, pet_visible: bool) {
        let _ = self.requests.send(Request::Show { x, y, pet_visible });
    }

    pub fn poll_commands(&self) -> impl Iterator<Item = u32> + '_ {
        self.commands.try_iter()
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

    let owner = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
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

    while let Ok(request) = requests.recv() {
        match request {
            Request::Show { x, y, pet_visible } => {
                let toggle = if pet_visible {
                    "隐藏宠物"
                } else {
                    "显示宠物"
                };
                let toggle_text = wide(toggle);
                unsafe {
                    let _ = ModifyMenuW(
                        menu,
                        COMMAND_TOGGLE_PET,
                        MF_BYCOMMAND | MF_STRING,
                        COMMAND_TOGGLE_PET as usize,
                        toggle_text.as_ptr(),
                    );
                    let command = TrackPopupMenuEx(
                        menu,
                        TPM_RETURNCMD | TPM_NONOTIFY | TPM_WORKAREA,
                        x,
                        y,
                        owner,
                        ptr::null(),
                    );
                    if command != 0 {
                        let _ = commands.send(command as u32);
                        wake.request_repaint();
                    }
                }
            }
            Request::Shutdown => break,
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
