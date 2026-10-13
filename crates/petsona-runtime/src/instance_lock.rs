use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// A process-scoped lock for one Petsona data directory.
///
/// The lock is advisory at the OS level and is released automatically when
/// the file handle is dropped, including when the process exits unexpectedly.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    pub fn acquire(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(std::fs::TryLockError::WouldBlock) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "another Petsona instance is already running",
            )),
            Err(std::fs::TryLockError::Error(error)) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn lock_is_exclusive_until_owner_is_dropped() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "petsona-instance-lock-{}-{nonce}.lock",
            std::process::id()
        ));

        let first = InstanceLock::acquire(&path).expect("first owner acquires the lock");
        let second = InstanceLock::acquire(&path).expect_err("second owner must be rejected");
        assert_eq!(second.kind(), io::ErrorKind::AlreadyExists);
        drop(first);

        let third = InstanceLock::acquire(&path).expect("lock is released with its owner");
        drop(third);
        let _ = std::fs::remove_file(path);
    }
}
