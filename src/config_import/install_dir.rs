use std::path::{Path, PathBuf};

use super::error::TomlParseError;

#[derive(Debug, hbb_common::thiserror::Error)]
pub enum InstallDirError {
    #[error("安装目录不存在: {0}")]
    NotFound(String),
    #[error("无法获取可执行文件路径: {0}")]
    ExePathError(String),
}

impl From<InstallDirError> for super::error::ConfigImportError {
    fn from(e: InstallDirError) -> Self {
        super::error::ConfigImportError::InstallDirNotFound(e.to_string())
    }
}

pub struct InstallDirDetector;

impl InstallDirDetector {
    pub fn detect_toml_config() -> Option<PathBuf> {
        let dir = match Self::get_install_dir() {
            Ok(d) => d,
            Err(e) => {
                log::info!("安装目录检测失败: {}", e);
                return None;
            }
        };
        let toml_path = dir.join("rustdesk.toml");
        if toml_path.is_file() && validate_path_safety(&toml_path).is_ok() {
            Some(toml_path)
        } else {
            None
        }
    }

    pub fn get_install_dir() -> Result<PathBuf, InstallDirError> {
        #[cfg(target_os = "windows")]
        {
            if let Some(dir) = get_install_dir_from_registry() {
                return Ok(dir);
            }
            return current_exe_parent();
        }
        #[cfg(target_os = "macos")]
        {
            let candidates = [
                Path::new("/Applications/RustDesk.app/Contents/MacOS"),
            ];
            for c in candidates {
                if c.is_dir() {
                    return Ok(c.to_path_buf());
                }
            }
            return current_exe_parent();
        }
        #[cfg(target_os = "linux")]
        {
            let candidates = [
                Path::new("/usr/bin/rustdesk"),
                Path::new("/opt/rustdesk"),
            ];
            for c in candidates {
                if c.is_dir() {
                    return Ok(c.to_path_buf());
                }
            }
            if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
                for dir in xdg.split(':') {
                    let candidate = Path::new(dir).join("rustdesk");
                    if candidate.is_dir() {
                        return Ok(candidate);
                    }
                }
            }
            return current_exe_parent();
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            current_exe_parent()
        }
    }
}

pub(crate) fn validate_path_safety(path: &Path) -> Result<(), TomlParseError> {
    let path_str = path.to_string_lossy();
    if path_str.contains("..") {
        return Err(TomlParseError::PathSecurityError(format!(
            "路径包含非法跳转: {}",
            path_str
        )));
    }
    #[cfg(unix)]
    {
        let sensitive = ["/etc/shadow", "/etc/passwd"];
        for s in sensitive {
            if path_str == s {
                return Err(TomlParseError::PathSecurityError(format!(
                    "敏感系统文件: {}",
                    path_str
                )));
            }
        }
    }
    Ok(())
}

fn current_exe_parent() -> Result<PathBuf, InstallDirError> {
    let exe = std::env::current_exe()
        .map_err(|e| InstallDirError::ExePathError(e.to_string()))?;
    Ok(exe.parent().unwrap_or(Path::new(".")).to_path_buf())
}

#[cfg(target_os = "windows")]
fn get_install_dir_from_registry() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let subkey = hklm
        .open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\RustDesk")
        .ok()?;
    let install_location: String = subkey.get_value("InstallLocation").ok()?;
    let path = PathBuf::from(install_location);
    if path.is_dir() {
        Some(path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_safety_normal() {
        let path = Path::new("/usr/bin/rustdesk/rustdesk.toml");
        assert!(validate_path_safety(path).is_ok());
    }

    #[test]
    fn test_validate_path_safety_traversal() {
        let path = Path::new("/usr/bin/../etc/rustdesk.toml");
        assert!(matches!(
            validate_path_safety(path),
            Err(TomlParseError::PathSecurityError(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_validate_path_safety_sensitive() {
        let path = Path::new("/etc/passwd");
        assert!(matches!(
            validate_path_safety(path),
            Err(TomlParseError::PathSecurityError(_))
        ));
    }

    #[test]
    fn test_detect_toml_config_missing() {
        let res = InstallDirDetector::detect_toml_config();
        let _ = res;
    }
}
