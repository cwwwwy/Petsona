use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

const LABEL: &str = "com.petsona.desktop";
const PLIST_TEMPLATE: &str = include_str!("../../../packaging/macos/com.petsona.desktop.plist");

#[cfg(feature = "test-hooks")]
const TEST_DIRECTORY_ENV: &str = "PETSONA_AUTOSTART_PLIST_DIR";

pub(crate) fn is_enabled() -> bool {
    plist_path().is_ok_and(|path| path.is_file())
}

pub(crate) fn set_enabled(enabled: bool) -> Result<(), String> {
    let plist_path = plist_path()?;
    if !enabled {
        return match fs::remove_file(&plist_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("无法移除开机自启动配置：{error}")),
        };
    }

    let executable =
        env::current_exe().map_err(|error| format!("无法读取 Petsona 可执行文件路径：{error}"))?;
    let working_directory = executable
        .parent()
        .ok_or_else(|| "无法确定 Petsona 的工作目录".to_string())?;
    let executable = executable
        .to_str()
        .ok_or_else(|| "Petsona 可执行文件路径不是有效的 UTF-8".to_string())?;
    let working_directory = working_directory
        .to_str()
        .ok_or_else(|| "Petsona 工作目录不是有效的 UTF-8".to_string())?;
    let plist = render_plist(executable, working_directory);

    let directory = plist_path
        .parent()
        .ok_or_else(|| "无法确定 LaunchAgents 目录".to_string())?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("无法创建 LaunchAgents 目录：{error}"))?;

    let temporary_path = directory.join(format!(".{LABEL}.{}.tmp", std::process::id()));
    fs::write(&temporary_path, plist)
        .map_err(|error| format!("无法写入开机自启动配置：{error}"))?;
    fs::set_permissions(&temporary_path, fs::Permissions::from_mode(0o644))
        .map_err(|error| format!("无法设置开机自启动配置权限：{error}"))?;

    let validation = Command::new("/usr/bin/plutil")
        .arg("-lint")
        .arg(&temporary_path)
        .output()
        .map_err(|error| format!("无法验证开机自启动配置：{error}"))?;
    if !validation.status.success() {
        let _ = fs::remove_file(&temporary_path);
        let detail = String::from_utf8_lossy(&validation.stderr);
        return Err(format!("开机自启动配置无效：{}", detail.trim()));
    }

    if let Err(error) = fs::rename(&temporary_path, &plist_path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(format!("无法启用开机自启动：{error}"));
    }

    // LaunchAgents in this directory are loaded at the next Aqua login. Do not
    // bootstrap the job here: RunAtLoad would immediately launch a second copy
    // of the app while the settings window is already open.
    Ok(())
}

fn plist_path() -> Result<PathBuf, String> {
    #[cfg(feature = "test-hooks")]
    if let Some(directory) = env::var_os(TEST_DIRECTORY_ENV) {
        return Ok(PathBuf::from(directory).join(format!("{LABEL}.plist")));
    }

    let home = env::var_os("HOME").ok_or_else(|| "无法找到当前用户主目录".to_string())?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

fn render_plist(executable: &str, working_directory: &str) -> String {
    PLIST_TEMPLATE
        .replace("@EXECUTABLE@", &escape_xml(executable))
        .replace("@WORKING_DIRECTORY@", &escape_xml(working_directory))
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::{escape_xml, render_plist};

    #[test]
    fn launch_agent_plist_embeds_executable_and_working_directory() {
        let plist = render_plist(
            "/Applications/Petsona.app/Contents/MacOS/Petsona",
            "/Applications/Petsona.app/Contents/MacOS",
        );

        assert!(plist.contains("<string>com.petsona.desktop</string>"));
        assert!(plist.contains("<string>/Applications/Petsona.app/Contents/MacOS/Petsona</string>"));
        assert!(plist.contains("<string>/Applications/Petsona.app/Contents/MacOS</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>\n\t<true/>"));
        assert!(!plist.contains('@'));
    }

    #[test]
    fn launch_agent_paths_are_xml_escaped() {
        let plist = render_plist(
            "/Applications/A&B <Pets>/Petsona",
            "/Applications/A&B <Pets>",
        );

        assert!(plist.contains("/Applications/A&amp;B &lt;Pets&gt;/Petsona"));
        assert!(!plist.contains("/Applications/A&B <Pets>"));
        assert_eq!(escape_xml("a'b\"c"), "a&apos;b&quot;c");
    }
}
