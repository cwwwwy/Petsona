use petsona_core::config::AppPaths;
use petsona_runtime::log_file::LogFile;
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static FILE: OnceLock<Option<LogFile>> = OnceLock::new();

pub fn mark_start() {
    let _ = START.set(Instant::now());
}

/// Keep shell diagnostics beside runtime logs; tests can provide a private path.
pub fn log(message: &str) {
    let elapsed = START.get().map(|t| t.elapsed().as_millis()).unwrap_or(0);
    let message = petsona_core::secrets::redact(message);
    let line = format!("[+{elapsed}ms] {message}\n");
    let file = FILE.get_or_init(|| {
        let path = std::env::var_os("PETSONA_SHELL_LOG")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| AppPaths::default().logs_dir.join("shell.log"));
        LogFile::new(path).ok()
    });
    if let Some(file) = file {
        let _ = file.append(&line);
    }
    #[cfg(debug_assertions)]
    eprint!("{line}");
}
