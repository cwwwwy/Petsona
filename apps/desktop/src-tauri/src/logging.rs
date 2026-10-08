use std::io::Write;
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

/// Records shell start time; call once at the very beginning of main.
pub fn mark_start() {
    let _ = START.set(Instant::now());
}

/// Appends a line to %TEMP%/petsona-desktop.log with elapsed ms since mark_start.
pub fn log(message: &str) {
    let elapsed = START.get().map(|t| t.elapsed().as_millis()).unwrap_or(0);
    let line = format!("[+{elapsed}ms] {message}\n");
    let path = std::env::temp_dir().join("petsona-desktop.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = file.write_all(line.as_bytes());
    }
    #[cfg(debug_assertions)]
    eprint!("{line}");
}
