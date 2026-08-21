use hbb_common::thiserror;

#[derive(Debug, thiserror::Error)]
pub enum ConfigImportError {
    #[error("安装目录不存在: {0}")]
    InstallDirNotFound(String),

    #[error("TOML 配置文件不存在")]
    TomlConfigNotFound,

    #[error("TOML 解析错误: {0}")]
    TomlParseError(#[from] TomlParseError),

    #[error("权限不足: {0}")]
    PermissionDenied(String),

    #[error("配置存储错误: {0}")]
    StoreError(String),

    #[error("未知错误: {0}")]
    Unknown(String),
}

#[derive(Debug, thiserror::Error)]
pub enum TomlParseError {
    #[error("文件读取失败: {0}")]
    FileReadError(#[from] std::io::Error),

    #[error("TOML 语法错误: 第 {line} 行, 第 {column} 列: {message}")]
    SyntaxError {
        line: usize,
        column: usize,
        message: String,
    },

    #[error("文件大小超限: 最大 {max}MB, 实际 {actual}MB")]
    FileSizeExceeded { max: u64, actual: u64 },

    #[error("编码错误: 期望 UTF-8 编码")]
    EncodingError,

    #[error("路径安全错误: {0}")]
    PathSecurityError(String),
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
