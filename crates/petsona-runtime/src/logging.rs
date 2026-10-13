use crate::log_file::LogFile;
use std::path::Path;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

/// Install bounded file logging. Each tracing event is buffered and redacted.
pub fn init(logs_dir: &Path) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let log_path = logs_dir.join("petsona.log");
    match LogFile::new(log_path.clone()) {
        Ok(file) => {
            let file = Arc::new(file);
            let _ = tracing_subscriber::fmt()
                .with_ansi(false)
                .with_writer(move || file.writer())
                .with_env_filter(filter)
                .try_init();
            tracing::info!(path = %log_path.display(), "file logging enabled");
        }
        Err(_) => {
            // Never turn a logging failure into an application failure or print
            // dynamic request details to an unfiltered fallback sink.
            eprintln!("cannot open Petsona log file");
        }
    }
}
