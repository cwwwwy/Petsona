//! Login-only LaunchAgent. No launchctl bootstrap/bootout: toggling this option
//! must neither launch a second instance now nor kill a login-started instance.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_LABEL: &str = "com.petsona.desktop";

struct Agent {
    directory: PathBuf,
    label: String,
}

impl Agent {
    fn new(directory: PathBuf, label: String) -> Result<Self, String> {
        if label.is_empty()
            || !label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || label.starts_with('.')
        {
            return Err("自启标签必须由字母、数字、点、下划线或连字符组成".into());
        }
        if !directory.is_absolute() {
            return Err("自启目录必须是绝对路径".into());
        }
        Ok(Self { directory, label })
    }

    fn path(&self) -> PathBuf {
        self.directory.join(format!("{}.plist", self.label))
    }

    fn enabled(&self) -> bool {
        self.path().is_file()
    }

    fn set(
        &self,
        enabled: bool,
        executable: &Path,
        home: Option<&Path>,
        service: Option<&str>,
    ) -> Result<(), String> {
        if !enabled {
            return match fs::remove_file(self.path()) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!("无法删除登录自启项：{error}")),
            };
        }
        let content = plist(
            &self.label,
            executable,
            home,
            service,
            Some(&self.directory),
        )?;
        fs::create_dir_all(&self.directory)
            .map_err(|error| format!("无法创建自启目录：{error}"))?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let temporary = self.directory.join(format!(
            ".{}-{}-{nonce}.tmp",
            self.label,
            std::process::id()
        ));
        let result = (|| -> std::io::Result<()> {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, self.path())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|error| format!("无法写入登录自启项：{error}"))
    }
}

fn from_environment() -> Result<Agent, String> {
    let directory = match std::env::var_os("PETSONA_AUTOSTART_DIR") {
        Some(path) => PathBuf::from(path),
        None => {
            let home = std::env::var_os("HOME").ok_or("无法确定用户主目录")?;
            PathBuf::from(home).join("Library/LaunchAgents")
        }
    };
    let label = std::env::var("PETSONA_AUTOSTART_LABEL").unwrap_or_else(|_| DEFAULT_LABEL.into());
    Agent::new(directory, label)
}

fn xml(value: &str) -> Result<String, String> {
    if value
        .chars()
        .any(|c| c < '\u{20}' && !matches!(c, '\t' | '\n' | '\r'))
    {
        return Err("自启路径或环境值含不支持的控制字符".into());
    }
    Ok(value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;"))
}

fn path_text(path: &Path) -> Result<&str, String> {
    if !path.is_absolute() {
        return Err("登录自启路径必须是绝对路径".into());
    }
    path.to_str()
        .ok_or_else(|| "登录自启路径必须是有效 UTF-8".into())
}

fn plist(
    label: &str,
    executable: &Path,
    home: Option<&Path>,
    service: Option<&str>,
    agent_directory: Option<&Path>,
) -> Result<String, String> {
    let executable = xml(path_text(executable)?)?;
    let mut environment = String::new();
    if let Some(home) = home {
        environment.push_str(&format!(
            "<key>PETSONA_HOME</key><string>{}</string>\n",
            xml(path_text(home)?)?
        ));
    }
    if let Some(service) = service {
        environment.push_str(&format!(
            "<key>PETSONA_CREDENTIAL_SERVICE</key><string>{}</string>\n",
            xml(service)?
        ));
    }
    if let Some(directory) = agent_directory {
        environment.push_str(&format!(
            "<key>PETSONA_AUTOSTART_DIR</key><string>{}</string>\n<key>PETSONA_AUTOSTART_LABEL</key><string>{}</string>\n",
            xml(path_text(directory)?)?, xml(label)?
        ));
    }
    // Preserve isolation controls for a custom login instance, but never copy
    // model API keys or arbitrary environment variables into a public plist.
    let environment = if environment.is_empty() {
        String::new()
    } else {
        format!("<key>EnvironmentVariables</key><dict>{environment}</dict>\n")
    };
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{}</string>
<key>ProgramArguments</key><array><string>{executable}</string></array>
<key>RunAtLoad</key><true/>
<key>KeepAlive</key><false/>
<key>ProcessType</key><string>Interactive</string>
{environment}</dict></plist>
"#,
        xml(label)?
    ))
}

pub fn is_enabled() -> bool {
    from_environment()
        .map(|agent| agent.enabled())
        .unwrap_or(false)
}

pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let agent = from_environment()?;
    let executable =
        std::env::current_exe().map_err(|error| format!("无法读取应用路径：{error}"))?;
    let home = std::env::var_os("PETSONA_HOME").map(PathBuf::from);
    let service = std::env::var("PETSONA_CREDENTIAL_SERVICE").ok();
    agent.set(enabled, &executable, home.as_deref(), service.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_agent_roundtrip_preserves_spaces_unicode_and_environment() {
        let directory = std::env::temp_dir().join(format!(
            "petsona-agent-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let agent = Agent::new(directory.clone(), "com.petsona.accept".into()).unwrap();
        let executable = Path::new("/tmp/宠物 & <伙伴>/Petsona.app/Contents/MacOS/petsona-desktop");
        let home = Path::new("/tmp/隔离 & 数据");
        assert!(!agent.enabled());
        agent
            .set(
                true,
                executable,
                Some(home),
                Some("com.petsona.accept-keyring"),
            )
            .unwrap();
        let output = std::process::Command::new("plutil")
            .args(["-lint"])
            .arg(agent.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let output = std::process::Command::new("plutil")
            .args(["-extract", "ProgramArguments.0", "raw", "-o", "-"])
            .arg(agent.path())
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim_end(),
            executable.to_str().unwrap()
        );
        assert!(agent.enabled());
        agent
            .set(true, Path::new("/tmp/updated/Petsona"), None, None)
            .unwrap();
        assert!(!fs::read_to_string(agent.path())
            .unwrap()
            .contains("PETSONA_HOME"));
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        agent.set(false, executable, None, None).unwrap();
        agent.set(false, executable, None, None).unwrap();
        assert!(!agent.enabled());
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn invalid_label_or_path_cannot_escape_agent_directory() {
        for label in ["", "../Petsona", "/tmp/other", "com.petsona/other", "a\\b"] {
            assert!(Agent::new(PathBuf::from("/tmp/isolated"), label.into()).is_err());
        }
        assert!(Agent::new(PathBuf::from("relative"), DEFAULT_LABEL.into()).is_err());
        assert!(plist(DEFAULT_LABEL, Path::new("relative"), None, None, None).is_err());
        assert!(xml("bad\0path").is_err());
    }
}
