use std::path::Path;

use super::error::TomlParseError;

/// Reject paths that must never be treated as a config to import.
///
/// The path comes straight from the command line, so a `..` segment or a sensitive system file
/// has to be refused before anything is read from it.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_safety_normal() {
        let path = Path::new("/usr/bin/rustdesk/rustdesk-config-import.toml");
        assert!(validate_path_safety(path).is_ok());
    }

    #[test]
    fn test_validate_path_safety_traversal() {
        let path = Path::new("/usr/bin/../etc/rustdesk-config-import.toml");
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
}
