#[derive(Debug)]
pub enum ConfigImportError {
    InstallDirNotFound(String),
    TomlConfigNotFound,
    TomlParseError(TomlParseError),
    PermissionDenied(String),
    StoreError(String),
    Unknown(String),
}

impl std::fmt::Display for ConfigImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigImportError::InstallDirNotFound(s) => write!(f, "安装目录不存在: {}", s),
            ConfigImportError::TomlConfigNotFound => write!(f, "TOML 配置文件不存在"),
            ConfigImportError::TomlParseError(e) => write!(f, "TOML 解析错误: {}", e),
            ConfigImportError::PermissionDenied(s) => write!(f, "权限不足: {}", s),
            ConfigImportError::StoreError(s) => write!(f, "配置存储错误: {}", s),
            ConfigImportError::Unknown(s) => write!(f, "未知错误: {}", s),
        }
    }
}

impl std::error::Error for ConfigImportError {}

impl From<TomlParseError> for ConfigImportError {
    fn from(e: TomlParseError) -> Self {
        ConfigImportError::TomlParseError(e)
    }
}

#[derive(Debug)]
pub enum TomlParseError {
    FileReadError(std::io::Error),
    SyntaxError {
        line: usize,
        column: usize,
        message: String,
    },
    FileSizeExceeded { max: u64, actual: u64 },
    EncodingError,
    PathSecurityError(String),
}

impl std::fmt::Display for TomlParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TomlParseError::FileReadError(e) => write!(f, "文件读取失败: {}", e),
            TomlParseError::SyntaxError {
                line,
                column,
                message,
            } => write!(f, "TOML 语法错误: 第 {} 行, 第 {} 列: {}", line, column, message),
            TomlParseError::FileSizeExceeded { max, actual } => {
                write!(f, "文件大小超限: 最大 {}MB, 实际 {}MB", max, actual)
            }
            TomlParseError::EncodingError => write!(f, "编码错误: 期望 UTF-8 编码"),
            TomlParseError::PathSecurityError(s) => write!(f, "路径安全错误: {}", s),
        }
    }
}

impl std::error::Error for TomlParseError {}

impl From<std::io::Error> for TomlParseError {
    fn from(e: std::io::Error) -> Self {
        TomlParseError::FileReadError(e)
    }
}

impl From<std::io::Error> for ConfigImportError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied(e.to_string()),
            _ => Self::StoreError(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_io_error_permission_denied() {
        let err = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let mapped = ConfigImportError::from(err);
        assert!(matches!(mapped, ConfigImportError::PermissionDenied(_)));
    }

    #[test]
    fn test_from_io_error_other() {
        let err = std::io::Error::from(std::io::ErrorKind::NotFound);
        let mapped = ConfigImportError::from(err);
        assert!(matches!(mapped, ConfigImportError::StoreError(_)));
    }

    #[test]
    fn test_from_toml_parse_error() {
        let err = TomlParseError::EncodingError;
        let mapped = ConfigImportError::from(err);
        assert!(matches!(mapped, ConfigImportError::TomlParseError(_)));
    }
}
