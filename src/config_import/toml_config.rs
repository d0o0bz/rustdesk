use std::collections::HashMap;

use serde::{Deserialize, Serialize};

fn default_version() -> String {
    "1.0".to_string()
}

/// The release of the schema this reads. A major version nobody here speaks has to be refused
/// rather than half understood: every field is optional, so a layout this does not recognise
/// reads as a file asking for nothing and leaves the machine silently unconfigured.
pub(crate) const SUPPORTED_VERSION: &str = "1.0";

fn major(version: &str) -> &str {
    version.split('.').next().unwrap_or(version)
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct TomlConfig {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub rendezvous_server: String,
    #[serde(default)]
    pub rendezvous_port: i32,
    #[serde(default)]
    pub relay_server: String,
    #[serde(default)]
    pub relay_port: i32,
    #[serde(default)]
    pub api_server: String,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub options: HashMap<String, String>,
    #[serde(default)]
    pub rendezvous_servers: Vec<TomlServerEntry>,
}

impl Default for TomlConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            rendezvous_server: String::new(),
            rendezvous_port: 0,
            relay_server: String::new(),
            relay_port: 0,
            api_server: String::new(),
            security: SecurityConfig::default(),
            network: NetworkConfig::default(),
            display: DisplayConfig::default(),
            options: HashMap::new(),
            rendezvous_servers: Vec::new(),
        }
    }
}

impl TomlConfig {
    pub fn is_empty(&self) -> bool {
        self.rendezvous_server.is_empty()
            && self.rendezvous_port == 0
            && self.relay_server.is_empty()
            && self.relay_port == 0
            && self.api_server.is_empty()
            && self.security == SecurityConfig::default()
            && self.network == NetworkConfig::default()
            && self.display == DisplayConfig::default()
            && self.options.is_empty()
            && self.rendezvous_servers.is_empty()
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    /// The version the file asks for, when this cannot read it.
    pub fn unsupported_version(&self) -> Option<&str> {
        let version = self.version();
        (major(version) != major(SUPPORTED_VERSION)).then_some(version)
    }
}

/// What a secret stands for in a log line, keeping a `Debug` of any struct holding one from
/// printing it.
fn secret(value: &str) -> &'static str {
    if value.is_empty() {
        ""
    } else {
        "***"
    }
}

#[derive(Default, Deserialize, Serialize, Clone, PartialEq)]
pub struct SecurityConfig {
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub access_mode: String,
    /// `None` when the file says nothing, so an explicit `false` can turn the setting off
    /// instead of being read as "leave it alone".
    #[serde(default)]
    pub enable_2fa: Option<bool>,
    #[serde(default)]
    pub whitelist_enabled: Option<bool>,
    #[serde(default)]
    pub whitelist: Vec<String>,
}

impl std::fmt::Debug for SecurityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecurityConfig")
            .field("password", &secret(&self.password))
            .field("access_mode", &self.access_mode)
            .field("enable_2fa", &self.enable_2fa)
            .field("whitelist_enabled", &self.whitelist_enabled)
            .field("whitelist", &self.whitelist)
            .finish()
    }
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq)]
pub struct NetworkConfig {
    #[serde(default)]
    pub network_type: String,
    #[serde(default)]
    pub proxy: ProxyConfig,
}

#[derive(Default, Deserialize, Serialize, Clone, PartialEq)]
pub struct ProxyConfig {
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub port: i32,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

impl std::fmt::Debug for ProxyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyConfig")
            .field("address", &self.address)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &secret(&self.password))
            .finish()
    }
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq)]
pub struct DisplayConfig {
    #[serde(default)]
    pub image_quality: String,
    #[serde(default)]
    pub view_style: String,
    #[serde(default)]
    pub scroll_style: String,
    #[serde(default)]
    pub show_remote_cursor: Option<bool>,
    #[serde(default)]
    pub disable_audio: Option<bool>,
    #[serde(default)]
    pub disable_clipboard: Option<bool>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq)]
pub struct TomlServerEntry {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub id_server: String,
    #[serde(default)]
    pub id_port: i32,
    #[serde(default)]
    pub relay_server: Option<String>,
    #[serde(default)]
    pub relay_port: Option<i32>,
    #[serde(default)]
    pub api_server: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub is_default: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_version() {
        let cfg = TomlConfig::default();
        assert_eq!(cfg.version(), "1.0");
        assert!(cfg.is_empty());
    }

    #[test]
    fn test_empty_toml() {
        let cfg: TomlConfig = hbb_common::toml::from_str("").unwrap();
        assert_eq!(cfg.version(), "1.0");
        assert!(cfg.is_empty());
    }

    #[test]
    fn test_version_within_the_supported_release() {
        assert_eq!(TomlConfig::default().unsupported_version(), None);
        let cfg: TomlConfig = hbb_common::toml::from_str("version = \"1.2\"\n").unwrap();
        assert_eq!(cfg.unsupported_version(), None);
    }

    #[test]
    fn test_unsupported_major_version() {
        let cfg: TomlConfig = hbb_common::toml::from_str("version = \"2.0\"\n").unwrap();
        assert_eq!(cfg.unsupported_version(), Some("2.0"));
    }

    #[test]
    fn test_secret_is_not_printed() {
        let cfg: TomlConfig = hbb_common::toml::from_str(
            "[security]\npassword = \"pw\"\n[network.proxy]\npassword = \"pp\"\n",
        )
        .unwrap();
        let printed = format!("{:?}", cfg.security);
        assert!(!printed.contains("pw"), "{}", printed);
        assert!(!format!("{:?}", cfg.network.proxy).contains("pp"));
    }

    #[test]
    fn test_full_deserialize() {
        let content = r#"
rendezvous_server = "rs1.rustdesk.com"
rendezvous_port = 21116
relay_server = "relay.rustdesk.com"
relay_port = 21117
api_server = "api.rustdesk.com"

[security]
password = "your_password"
access_mode = "full"
enable_2fa = false
whitelist_enabled = true
whitelist = ["device_id_1", "device_id_2"]

[network]
network_type = "direct"

[network.proxy]
address = "127.0.0.1"
port = 1080
username = "user"
password = "proxy_password"

[display]
image_quality = "high"
view_style = "scroll"
scroll_style = "auto"
show_remote_cursor = true
disable_audio = false
disable_clipboard = false

[options]
custom_resolution = "1920x1080"
"#;
        let cfg: TomlConfig = hbb_common::toml::from_str(content).unwrap();
        assert_eq!(cfg.rendezvous_server, "rs1.rustdesk.com");
        assert_eq!(cfg.rendezvous_port, 21116);
        assert_eq!(cfg.relay_server, "relay.rustdesk.com");
        assert_eq!(cfg.api_server, "api.rustdesk.com");
        assert_eq!(cfg.security.password, "your_password");
        assert_eq!(cfg.security.access_mode, "full");
        assert_eq!(cfg.security.enable_2fa, Some(false));
        assert_eq!(cfg.security.whitelist_enabled, Some(true));
        assert_eq!(cfg.security.whitelist, vec!["device_id_1", "device_id_2"]);
        assert_eq!(cfg.network.network_type, "direct");
        assert_eq!(cfg.network.proxy.address, "127.0.0.1");
        assert_eq!(cfg.network.proxy.port, 1080);
        assert_eq!(cfg.display.image_quality, "high");
        assert_eq!(cfg.display.show_remote_cursor, Some(true));
        assert_eq!(cfg.options.get("custom_resolution").unwrap(), "1920x1080");
        assert!(!cfg.is_empty());
    }

    #[test]
    fn test_partial_deserialize() {
        let content = r#"
rendezvous_server = "rs.example.com"
"#;
        let cfg: TomlConfig = hbb_common::toml::from_str(content).unwrap();
        assert_eq!(cfg.rendezvous_server, "rs.example.com");
        assert!(!cfg.is_empty());
    }

    #[test]
    fn test_multi_rendezvous_servers() {
        let content = r#"
[[rendezvous_servers]]
name = "Server 1"
id_server = "rs1.rustdesk.com"
id_port = 21116
relay_server = "relay1.rustdesk.com"
relay_port = 21117
api_server = "api1.rustdesk.com"
key = "pubkey1"
is_default = true

[[rendezvous_servers]]
name = "Server 2"
id_server = "rs2.rustdesk.com"
relay_server = "relay2.rustdesk.com"
"#;
        let cfg: TomlConfig = hbb_common::toml::from_str(content).unwrap();
        assert_eq!(cfg.rendezvous_servers.len(), 2);
        assert_eq!(cfg.rendezvous_servers[0].name, "Server 1");
        assert_eq!(cfg.rendezvous_servers[0].id_server, "rs1.rustdesk.com");
        assert_eq!(cfg.rendezvous_servers[0].id_port, 21116);
        assert_eq!(
            cfg.rendezvous_servers[0].relay_server.as_deref(),
            Some("relay1.rustdesk.com")
        );
        assert_eq!(
            cfg.rendezvous_servers[0].api_server.as_deref(),
            Some("api1.rustdesk.com")
        );
        assert_eq!(cfg.rendezvous_servers[0].key.as_deref(), Some("pubkey1"));
        assert!(cfg.rendezvous_servers[0].is_default);
        assert!(
            cfg.rendezvous_servers[1].key.is_none(),
            "key stays unset when the file omits it"
        );
        assert_eq!(cfg.rendezvous_servers[1].name, "Server 2");
        assert_eq!(cfg.rendezvous_servers[1].id_server, "rs2.rustdesk.com");
        assert!(!cfg.rendezvous_servers[1].is_default);
        assert!(!cfg.is_empty());
    }
}
