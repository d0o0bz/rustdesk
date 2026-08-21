use std::path::Path;

use super::error::TomlParseError;
use super::toml_config::TomlConfig;

const MAX_FILE_SIZE: u64 = 1 * 1024 * 1024;
const MAX_RETRIES: u32 = 3;
const RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

pub struct TomlConfigParser;

impl TomlConfigParser {
    pub fn parse(path: &Path) -> Result<TomlConfig, TomlParseError> {
        let size = std::fs::metadata(path)
            .map_err(TomlParseError::FileReadError)?
            .len();
        if size > MAX_FILE_SIZE {
            return Err(TomlParseError::FileSizeExceeded {
                max: 1,
                actual: size / (1024 * 1024) + 1,
            });
        }
        let content = read_with_retry(path)?;
        match hbb_common::toml::from_str::<TomlConfig>(&content) {
            Ok(cfg) => Ok(cfg),
            Err(e) => {
                let (line, column) = line_col_from_span(&content, e.span());
                Err(TomlParseError::SyntaxError {
                    line,
                    column,
                    message: e.to_string(),
                })
            }
        }
    }
}

fn read_with_retry(path: &Path) -> Result<String, TomlParseError> {
    let mut retries = 0u32;
    loop {
        match std::fs::read(path) {
            Ok(bytes) => {
                return String::from_utf8(bytes).map_err(|_| TomlParseError::EncodingError);
            }
            Err(e) => {
                let kind = e.kind();
                if (kind == std::io::ErrorKind::WouldBlock
                    || kind == std::io::ErrorKind::PermissionDenied)
                    && retries < MAX_RETRIES
                {
                    std::thread::sleep(RETRY_INTERVAL);
                    retries += 1;
                    continue;
                }
                return Err(TomlParseError::FileReadError(e));
            }
        }
    }
}

fn line_col_from_span(content: &str, span: Option<std::ops::Range<usize>>) -> (usize, usize) {
    match span {
        Some(range) => byte_offset_to_line_col(content, range.start),
        None => (0, 0),
    }
}

fn byte_offset_to_line_col(content: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in content.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(content: &[u8], suffix: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "rustdesk_toml_test_{}{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            suffix
        ));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_file_not_found() {
        let path = Path::new("/nonexistent/path/rustdesk_test.toml");
        let res = TomlConfigParser::parse(path);
        assert!(res.is_err());
        assert!(matches!(
            res.unwrap_err(),
            TomlParseError::FileReadError(_)
        ));
    }

    #[test]
    fn test_syntax_error() {
        let content = b"rendezvous_server = \n";
        let path = write_temp(content, ".toml");
        let res = TomlConfigParser::parse(&path);
        let _ = std::fs::remove_file(&path);
        assert!(res.is_err());
        match res.unwrap_err() {
            TomlParseError::SyntaxError { message, .. } => assert!(!message.is_empty()),
            other => panic!("expected SyntaxError, got {:?}", other),
        }
    }

    #[test]
    fn test_encoding_error() {
        let content = &[0xff, 0xfe, 0xfd];
        let path = write_temp(content, ".toml");
        let res = TomlConfigParser::parse(&path);
        let _ = std::fs::remove_file(&path);
        assert!(matches!(res.unwrap_err(), TomlParseError::EncodingError));
    }

    #[test]
    fn test_valid_parse() {
        let content = b"rendezvous_server = \"rs.example.com\"\n";
        let path = write_temp(content, ".toml");
        let res = TomlConfigParser::parse(&path);
        let _ = std::fs::remove_file(&path);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().rendezvous_server, "rs.example.com");
    }

    #[test]
    fn test_parse_performance() {
        let mut content = String::from("rendezvous_server = \"rs.example.com\"\n");
        let mut i = 0;
        while content.len() < 100 * 1024 {
            content.push_str(&format!("opt_key_{} = \"value\"\n", i));
            i += 1;
        }
        let path = write_temp(content.as_bytes(), ".toml");
        let start = std::time::Instant::now();
        let res = TomlConfigParser::parse(&path);
        let elapsed = start.elapsed();
        let _ = std::fs::remove_file(&path);
        assert!(res.is_ok());
        assert!(
            elapsed.as_millis() < 2000,
            "解析耗时 {}ms（release 模式应 < 500ms）",
            elapsed.as_millis()
        );
    }
}
