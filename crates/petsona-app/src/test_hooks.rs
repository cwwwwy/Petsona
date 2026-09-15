//! Feature-gated local control channel used by platform smoke tests.
//!
//! The server is compiled only with the `test-hooks` feature and refuses to
//! start unless `PETSONA_TEST_HOOKS=1` is set. It binds to loopback and
//! requires a per-run token, so a normal release build and a normal user
//! session keep the production protocol unchanged.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

const ENABLED_ENV: &str = "PETSONA_TEST_HOOKS";
const PORT_ENV: &str = "PETSONA_TEST_HOOKS_PORT";
const TOKEN_ENV: &str = "PETSONA_TEST_HOOKS_TOKEN";
const MAX_BODY_BYTES: usize = 8 * 1024;

/// Snapshot consumed by platform smoke scripts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TestStatus {
    pub ok: bool,
    pub version: String,
    pub process_id: u32,
    pub pet_visible: bool,
    pub settings_open: bool,
    pub settings_key_window: bool,
    pub menu_open: bool,
    pub click_through: bool,
    pub passthrough: bool,
    pub pointer_left_down: bool,
    pub always_on_top: bool,
    pub scale: f32,
    pub state: String,
    pub base_state: String,
    pub sprite_index: u32,
    pub bubble_text: Option<String>,
    pub bubble_window_created: bool,
    pub native_menu_ready: bool,
    pub gaze_side: i8,
    pub gaze_phase: Option<String>,
    pub pet_dragged: bool,
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub pet_click_x: Option<i32>,
    pub pet_click_y: Option<i32>,
    pub pet_click_hits: bool,
    pub logic_count: u64,
    pub ui_count: u64,
    pub state_event_count: u64,
    pub cursor_poll_count: u64,
    pub mouse_events: bool,
    pub mouse_position_valid: bool,
    pub style_reapply_count: u64,
    pub last_repaint_ms: u64,
    pub animation_repaint_ms: u64,
    pub repaint_fast: u64,
    pub repaint_medium: u64,
    pub repaint_slow: u64,
    pub hooks_port: u16,
}

impl Default for TestStatus {
    fn default() -> Self {
        Self {
            ok: true,
            version: env!("CARGO_PKG_VERSION").to_string(),
            process_id: std::process::id(),
            pet_visible: true,
            settings_open: false,
            settings_key_window: false,
            menu_open: false,
            click_through: true,
            passthrough: false,
            pointer_left_down: false,
            always_on_top: true,
            scale: 1.0,
            state: "idle".to_string(),
            base_state: "idle".to_string(),
            sprite_index: 0,
            bubble_text: None,
            bubble_window_created: false,
            native_menu_ready: false,
            gaze_side: 0,
            gaze_phase: None,
            pet_dragged: false,
            window_x: None,
            window_y: None,
            window_width: None,
            window_height: None,
            pet_click_x: None,
            pet_click_y: None,
            pet_click_hits: false,
            logic_count: 0,
            ui_count: 0,
            state_event_count: 0,
            cursor_poll_count: 0,
            mouse_events: false,
            mouse_position_valid: false,
            style_reapply_count: 0,
            last_repaint_ms: 0,
            animation_repaint_ms: 0,
            repaint_fast: 0,
            repaint_medium: 0,
            repaint_slow: 0,
            hooks_port: 0,
        }
    }
}

/// A deterministic action sent to the UI thread by the smoke harness.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TestActionRequest {
    pub action: String,
    pub enabled: Option<bool>,
    pub value: Option<f64>,
    pub text: Option<String>,
    pub ttl_ms: Option<u64>,
}

pub struct TestHookServer {
    port: u16,
    status: Arc<Mutex<TestStatus>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TestHookServer {
    /// Start the optional server when the test environment explicitly asks for it.
    pub fn start_from_env<F>(sender: Sender<TestActionRequest>, wake: F) -> Result<Option<Self>>
    where
        F: Fn() + Send + Sync + 'static,
    {
        if std::env::var(ENABLED_ENV).ok().as_deref() != Some("1") {
            return Ok(None);
        }

        let port = std::env::var(PORT_ENV)
            .with_context(|| format!("{PORT_ENV} must be set when test hooks are enabled"))?
            .parse::<u16>()
            .with_context(|| format!("{PORT_ENV} must be a TCP port"))?;
        let token = std::env::var(TOKEN_ENV)
            .with_context(|| format!("{TOKEN_ENV} must be set when test hooks are enabled"))?;
        if token.trim().is_empty() {
            bail!("{TOKEN_ENV} must not be empty");
        }

        Self::start(port, token, sender, wake).map(Some)
    }

    fn start<F>(
        port: u16,
        token: String,
        sender: Sender<TestActionRequest>,
        wake: F,
    ) -> Result<Self>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .with_context(|| format!("cannot bind the test hook server to 127.0.0.1:{port}"))?;
        let address = listener
            .local_addr()
            .context("cannot read the test hook server address")?;
        listener
            .set_nonblocking(true)
            .context("cannot configure the test hook server")?;

        let status = Arc::new(Mutex::new(TestStatus {
            hooks_port: address.port(),
            ..TestStatus::default()
        }));
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let thread_status = Arc::clone(&status);
        let wake = Arc::new(wake);

        let handle = thread::Builder::new()
            .name("petsona-test-hooks".to_string())
            .spawn(move || {
                run_server(
                    listener,
                    token,
                    sender,
                    thread_status,
                    wake,
                    thread_shutdown,
                );
            })
            .context("cannot start the test hook thread")?;

        Ok(Self {
            port: address.port(),
            status,
            shutdown,
            handle: Some(handle),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn status(&self) -> Arc<Mutex<TestStatus>> {
        Arc::clone(&self.status)
    }
}

impl Drop for TestHookServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = std::net::TcpStream::connect((Ipv4Addr::LOCALHOST, self.port));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run_server(
    listener: TcpListener,
    token: String,
    sender: Sender<TestActionRequest>,
    status: Arc<Mutex<TestStatus>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = handle_connection(stream, &token, &sender, &status, &wake);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

fn handle_connection(
    mut stream: std::net::TcpStream,
    token: &str,
    sender: &Sender<TestActionRequest>,
    status: &Arc<Mutex<TestStatus>>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();

    let mut content_length = 0usize;
    let mut request_token = String::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            match name.as_str() {
                "content-length" => {
                    content_length = value.parse().unwrap_or(0);
                }
                "x-petsona-token" => request_token = value.to_string(),
                _ => {}
            }
        }
    }

    if request_token != token {
        return write_response(&mut stream, 403, r#"{"ok":false,"error":"forbidden"}"#);
    }
    if content_length > MAX_BODY_BYTES {
        return write_response(&mut stream, 413, r#"{"ok":false,"error":"body too large"}"#);
    }

    let mut body = vec![0u8; content_length];
    if !body.is_empty() {
        reader.read_exact(&mut body)?;
    }

    let (status_code, response) = match (method, path) {
        ("GET", "/test/status") => {
            let snapshot = status
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_else(|poisoned| poisoned.into_inner().clone());
            match serde_json::to_string(&snapshot) {
                Ok(json) => (200, json),
                Err(error) => (500, format!(r#"{{"ok":false,"error":"{error}"}}"#)),
            }
        }
        ("POST", "/test/action") => match serde_json::from_slice::<TestActionRequest>(&body) {
            Ok(action) if !action.action.trim().is_empty() => {
                if sender.send(action).is_ok() {
                    wake();
                    (202, r#"{"ok":true}"#.to_string())
                } else {
                    (
                        503,
                        r#"{"ok":false,"error":"UI thread is gone"}"#.to_string(),
                    )
                }
            }
            Ok(_) => (400, r#"{"ok":false,"error":"action is empty"}"#.to_string()),
            Err(error) => (400, format!(r#"{{"ok":false,"error":"{error}"}}"#)),
        },
        _ => (404, r#"{"ok":false,"error":"not found"}"#.to_string()),
    };

    let reason = match status_code {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status_code} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{response}",
        response.len()
    )?;
    stream.flush()
}

fn write_response(
    stream: &mut std::net::TcpStream,
    status_code: u16,
    body: &str,
) -> std::io::Result<()> {
    let reason = match status_code {
        403 => "Forbidden",
        413 => "Payload Too Large",
        _ => "Bad Request",
    };
    write!(
        stream,
        "HTTP/1.1 {status_code} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_defaults_to_current_process() {
        let status = TestStatus::default();
        assert!(status.ok);
        assert_eq!(status.process_id, std::process::id());
    }

    #[test]
    fn action_parses_camel_case_fields() {
        let action: TestActionRequest =
            serde_json::from_str(r#"{"action":"show-bubble","text":"hello","ttlMs":250}"#)
                .expect("valid test action");
        assert_eq!(action.action, "show-bubble");
        assert_eq!(action.text.as_deref(), Some("hello"));
        assert_eq!(action.ttl_ms, Some(250));
    }
}
