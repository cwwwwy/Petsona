//! Secret storage: OS keychain first, restricted file fallback.
//!
//! API keys are never written to the app config file. The config only stores a
//! reference (`keyring:provider/<id>`) and the value lives in the platform
//! credential store.

use std::collections::HashMap;
use std::path::PathBuf;

use parking_lot::Mutex;

use crate::error::{Error, Result};

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// macOS Keychain / Windows Credential Manager / Secret Service.
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, key: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, key)
            .map_err(|e| Error::config(format!("cannot open OS keychain: {e}")))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        match self.entry(key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::config(format!("keychain read failed: {e}"))),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.entry(key)?
            .set_password(value)
            .map_err(|e| Error::config(format!("keychain write failed: {e}")))
    }

    fn delete(&self, key: &str) -> Result<()> {
        match self.entry(key)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::config(format!("keychain delete failed: {e}"))),
        }
    }
}

/// A file-backed fallback used when the OS keychain is unavailable.
///
/// The file is created with mode 0600 on Unix. This is strictly a fallback and
/// the UI must warn the user when it is in use.
pub struct FileSecretStore {
    path: PathBuf,
    cache: Mutex<HashMap<String, String>>,
}

impl FileSecretStore {
    pub fn new(path: PathBuf) -> Result<Self> {
        let store = Self {
            path,
            cache: Mutex::new(HashMap::new()),
        };
        store.reload()?;
        Ok(store)
    }

    fn reload(&self) -> Result<()> {
        if !self.path.is_file() {
            return Ok(());
        }
        let text = std::fs::read_to_string(&self.path)?;
        let map: HashMap<String, String> = serde_json::from_str(&text).unwrap_or_default();
        *self.cache.lock() = map;
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(&*self.cache.lock())?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

impl SecretStore for FileSecretStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.cache.lock().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.cache.lock().insert(key.to_string(), value.to_string());
        self.flush()
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.cache.lock().remove(key);
        self.flush()
    }
}

/// In-memory store for tests and for the CLI.
#[derive(Default)]
pub struct MemorySecretStore {
    map: Mutex<HashMap<String, String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.map.lock().get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.map.lock().insert(key.to_string(), value.to_string());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<()> {
        self.map.lock().remove(key);
        Ok(())
    }
}

/// Redact anything that looks like a credential from a string bound for logs.
pub fn redact(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut next_is_secret = false;
    for token in input.split_inclusive(char::is_whitespace) {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            out.push_str(token);
            continue;
        }
        let lower = trimmed
            .trim_start_matches(['"', '\'', '{'])
            .to_ascii_lowercase();
        let bearer = lower == "bearer";
        let label = ["x-api-key", "api_key", "apikey", "authorization"]
            .iter()
            .any(|name| {
                lower.starts_with(name)
                    && lower
                        .as_bytes()
                        .get(name.len())
                        .is_some_and(|c| matches!(c, b':' | b'=' | b'"'))
            });
        if next_is_secret || bearer || label || lower.starts_with("sk-") {
            out.push_str("***");
            out.push_str(&token[trimmed.len()..]);
            next_is_secret = bearer
                || (label
                    && (lower.trim_end_matches('"').ends_with([':', '='])
                        || lower.trim_end_matches('"').ends_with("bearer")));
        } else {
            out.push_str(token);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips() {
        let store = MemorySecretStore::new();
        assert!(store.get("a").unwrap().is_none());
        store.set("a", "secret").unwrap();
        assert_eq!(store.get("a").unwrap().as_deref(), Some("secret"));
        store.delete("a").unwrap();
        assert!(store.get("a").unwrap().is_none());
    }

    #[test]
    fn file_store_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(tmp.path().join("secrets.json")).unwrap();
        store.set("provider/deepseek", "sk-test").unwrap();
        let reopened = FileSecretStore::new(tmp.path().join("secrets.json")).unwrap();
        assert_eq!(
            reopened.get("provider/deepseek").unwrap().as_deref(),
            Some("sk-test")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(tmp.path().join("secrets.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn separated_header_values_and_json_keys_are_redacted_without_losing_whitespace() {
        let text = "Authorization: Bearer arbitrary-secret\nX-Api-Key: other-secret\n{\"api_key\": \"third-secret\"}\n";
        let clean = redact(text);
        for secret in ["arbitrary-secret", "other-secret", "third-secret"] {
            assert!(!clean.contains(secret));
        }
        assert_eq!(clean.lines().count(), 3);
        assert!(!redact("Authorization:Bearer attached-secret").contains("attached-secret"));
        assert!(!redact("{\"authorization\":\"Bearer quoted-secret\"}").contains("quoted-secret"));
        assert_eq!(redact("普通日志 key words\n"), "普通日志 key words\n");
    }

    #[test]
    fn redacts_credentials() {
        let redacted = redact("Authorization: Bearer sk-abc123 x-api-key: sk-def");
        assert!(!redacted.contains("sk-abc123"));
        assert!(!redacted.contains("sk-def"));
        assert!(redacted.contains("***"));
    }
}
