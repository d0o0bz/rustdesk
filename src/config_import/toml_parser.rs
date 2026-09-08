use std::io::Read;
use std::path::Path;

use super::error::TomlParseError;
use super::toml_config::TomlConfig;

const MB: u64 = 1024 * 1024;
const MAX_FILE_SIZE: u64 = MB;

/// The keys this reads at the top of the file, everything else being carried through, dropped,
/// or here used to spot a mistyped table name, see [`warn_unknown_keys`].
const KNOWN_TOP_LEVEL_KEYS: [&str; 11] = [
    "version",
    "rendezvous_server",
    "rendezvous_port",
    "relay_server",
    "relay_port",
    "api_server",
    "rendezvous_servers",
    "security",
    "network",
    "display",
    "options",
];

pub struct TomlConfigParser;

impl TomlConfigParser {
    pub fn parse(path: &Path) -> Result<TomlConfig, TomlParseError> {
        // One open for both the limit and the read. Checking the size through the path and
        // opening it again to read leaves a gap for whatever points at it to change, which is
        // the thing the limit is there to stop.
        let mut file = std::fs::File::open(path).map_err(TomlParseError::FileReadError)?;
        let size = file
            .metadata()
            .map_err(TomlParseError::FileReadError)?
            .len();
        if size > MAX_FILE_SIZE {
            return Err(size_exceeded(size));
        }
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::InvalidData => TomlParseError::EncodingError,
                _ => TomlParseError::FileReadError(e),
            })?;
        // Whatever grew past the cap between the two reads still has to be caught.
        if content.len() as u64 > MAX_FILE_SIZE {
            return Err(size_exceeded(content.len() as u64));
        }
        warn_unknown_keys(&content);
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

fn size_exceeded(size: u64) -> TomlParseError {
    TomlParseError::FileSizeExceeded {
        max: MAX_FILE_SIZE / MB,
        actual: (size + MB - 1) / MB,
    }
}

/// Name the top level keys that went unused.
///
/// Every key here is optional, so one that is mistyped reads as a file asking for nothing and
/// leaves no trace to find it by - the import reports success and changes nothing.
fn warn_unknown_keys(content: &str) {
    let Ok(value) = hbb_common::toml::from_str::<hbb_common::toml::Value>(content) else {
        return;
    };
    let Some(table) = value.as_table() else {
        return;
    };
    for key in table.keys() {
        if !KNOWN_TOP_LEVEL_KEYS.contains(&key.as_str()) {
            log::warn!("忽略未知的顶层配置项: {}", key);
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
    fn test_file_size_exceeded() {
        // Comments, so the file is valid TOML and the size is the only thing wrong with it.
        let content = vec![b'#'; MAX_FILE_SIZE as usize + 2];
        let path = write_temp(&content, ".toml");
        let res = TomlConfigParser::parse(&path);
        let _ = std::fs::remove_file(&path);
        assert!(matches!(
            res.unwrap_err(),
            TomlParseError::FileSizeExceeded { .. }
        ));
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
