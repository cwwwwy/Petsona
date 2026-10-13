//! Small bounded log sink shared by runtime tracing and the native shell.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

pub struct LogFile {
    path: PathBuf,
    max_bytes: u64,
    file: Mutex<Option<File>>,
}

impl LogFile {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        Self::with_limit(path, MAX_LOG_BYTES)
    }

    fn with_limit(path: PathBuf, max_bytes: u64) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = open(&path)?;
        Ok(Self {
            path,
            max_bytes,
            file: Mutex::new(Some(file)),
        })
    }

    pub fn append(&self, record: &str) -> io::Result<()> {
        let record = petsona_core::secrets::redact(record);
        let mut end = record.len().min(self.max_bytes as usize);
        while !record.is_char_boundary(end) {
            end -= 1;
        }
        let bytes = &record.as_bytes()[..end];
        let mut file = self
            .file
            .lock()
            .map_err(|_| io::Error::other("log lock poisoned"))?;
        if file.is_none() {
            *file = Some(open(&self.path)?);
        }
        if file
            .as_ref()
            .unwrap()
            .metadata()?
            .len()
            .saturating_add(bytes.len() as u64)
            > self.max_bytes
        {
            // Windows cannot rename a file while our writer retains its handle.
            drop(file.take());
            let backup = self.path.with_file_name(format!(
                "{}.1",
                self.path.file_name().unwrap_or_default().to_string_lossy()
            ));
            match fs::remove_file(&backup) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            fs::rename(&self.path, &backup)?;
            // An old unbounded log may already exceed the limit. Keep its tail,
            // then bound the single backup as well as the active file.
            let mut old = File::open(&backup)?;
            if old.metadata()?.len() > self.max_bytes {
                old.seek(SeekFrom::End(-(self.max_bytes as i64)))?;
                let mut tail = Vec::new();
                old.read_to_end(&mut tail)?;
                drop(old);
                let mut start = 0;
                while tail
                    .get(start)
                    .is_some_and(|byte| (0x80..=0xbf).contains(byte))
                {
                    start += 1;
                }
                fs::write(&backup, &tail[start..])?;
            }
            *file = Some(open(&self.path)?);
        }
        file.as_mut().unwrap().write_all(bytes)
    }

    pub fn writer(self: &Arc<Self>) -> LogWriter {
        LogWriter {
            sink: Arc::clone(self),
            bytes: Vec::new(),
        }
    }
}

fn open(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

/// Buffer one tracing event so redaction sees credentials split across writes.
pub struct LogWriter {
    sink: Arc<LogFile>,
    bytes: Vec<u8>,
}
impl Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let room = MAX_LOG_BYTES as usize - self.bytes.len();
        self.bytes
            .extend_from_slice(&bytes[..room.min(bytes.len())]);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if !self.bytes.is_empty() {
            let result = self.sink.append(&String::from_utf8_lossy(&self.bytes));
            self.bytes.clear();
            result?;
        }
        Ok(())
    }
}
impl Drop for LogWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "petsona-log-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn rotation_and_oversized_unicode_records_keep_only_two_bounded_files() {
        let dir = directory();
        let path = dir.join("shell.log");
        fs::write(&path, "伙伴".repeat(50)).unwrap();
        let sink = LogFile::with_limit(path.clone(), 32).unwrap();
        for _ in 0..4 {
            sink.append("first record\n").unwrap();
        }
        sink.append(&"伙伴".repeat(20)).unwrap();
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        for name in ["shell.log", "shell.log.1"] {
            assert!(fs::metadata(dir.join(name)).unwrap().len() <= 32);
            assert!(fs::read_to_string(dir.join(name)).is_ok());
        }
        drop(sink);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn event_fragments_are_redacted_together_and_concurrent_records_do_not_mix() {
        let dir = directory();
        let path = dir.join("runtime.log");
        let sink = Arc::new(LogFile::with_limit(path.clone(), 4096).unwrap());
        {
            let mut writer = sink.writer();
            writer.write_all(b"Authorization: Bearer ").unwrap();
            writer.write_all(b"ordinary-secret\n").unwrap();
        }
        assert!(!fs::read_to_string(&path)
            .unwrap()
            .contains("ordinary-secret"));
        let threads: Vec<_> = (0..8)
            .map(|i| {
                let sink = Arc::clone(&sink);
                std::thread::spawn(move || {
                    for _ in 0..16 {
                        sink.append(&format!("record-{i}\n")).unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(
            text.lines()
                .filter(|line| line.starts_with("record-"))
                .count(),
            128
        );
        drop(sink);
        fs::remove_dir_all(dir).unwrap();
    }
}
