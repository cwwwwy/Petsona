use std::fs::OpenOptions;
use std::path::Path;

use tracing_subscriber::fmt::writer::MakeWriterExt as _;
use tracing_subscriber::EnvFilter;

const LOG_FILE_NAME: &str = "petsona.log";

/// Install a file-backed subscriber while preserving terminal output for
/// `cargo run`. Release launches from Finder still retain a useful log file.
pub fn init(logs_dir: &Path) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let log_path = logs_dir.join(LOG_FILE_NAME);
    let file = OpenOptions::new().create(true).append(true).open(&log_path);

    match file {
        Ok(file) => {
            let writer = file.and(std::io::stderr);
            let _ = tracing_subscriber::fmt()
                .with_ansi(false)
                .with_writer(writer)
                .with_env_filter(filter)
                .try_init();
            tracing::info!(path = %log_path.display(), "file logging enabled");
        }
        Err(error) => {
            eprintln!(
                "cannot open Petsona log file {}: {error}",
                log_path.display()
            );
            let _ = tracing_subscriber::fmt()
                .with_ansi(false)
                .with_env_filter(filter)
                .try_init();
        }
    }
}
