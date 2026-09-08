use std::path::{Component, Path};

use super::error::TomlParseError;

/// Refuse a path that steps out of the directory it was aimed at.
///
/// The path comes straight from the command line, so leaving the directory has to be caught
/// before anything is read. Whole components rather than a search for `..` in the string,
/// which reads a perfectly ordinary `my..app` directory as an escape attempt. There is no list
/// of protected files to go with it either: without resolving the path such a list only
/// matches the one spelling it happens to know, and a file that is not valid TOML to begin
/// with gives nothing away.
pub(crate) fn validate_path_safety(path: &Path) -> Result<(), TomlParseError> {
    if path.components().any(|c| c == Component::ParentDir) {
        return Err(TomlParseError::PathSecurityError(format!(
            "路径包含上级目录跳转: {}",
            path.display()
        )));
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

    #[test]
    fn test_validate_path_safety_keeps_dots_in_a_name() {
        // `..` inside a name has nothing to do with leaving the directory.
        let path = Path::new("/opt/my..app/rustdesk..config.toml");
        assert!(validate_path_safety(path).is_ok());
    }
}
