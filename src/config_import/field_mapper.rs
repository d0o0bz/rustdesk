use std::collections::HashMap;

use hbb_common::config::{ServerConfig, Socks5Server};

use super::toml_config::{TomlConfig, TomlServerEntry};

pub struct FieldMapper;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct MappedConfig {
    pub password: Option<String>,
    pub rendezvous_server: Option<String>,
    pub rendezvous_servers: Vec<ServerConfig>,
    pub socks: Option<Socks5Server>,
    pub options: HashMap<String, String>,
}

impl FieldMapper {
    pub fn map_bool(value: bool) -> String {
        if value {
            "Y".to_string()
        } else {
            "N".to_string()
        }
    }

    pub fn map_list(value: &[String]) -> String {
        value.join(",")
    }

    pub fn map_optional_string(value: &str) -> Option<String> {
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    pub fn map_to_internal_config(toml_config: TomlConfig) -> MappedConfig {
        let mut options = HashMap::new();

        if let Some(v) = Self::map_optional_string(&toml_config.relay_server) {
            options.insert("relay-server".to_string(), v);
        }
        if let Some(v) = Self::map_optional_string(&toml_config.api_server) {
            options.insert("api-server".to_string(), v);
        }
        if let Some(v) = Self::map_optional_string(&toml_config.security.access_mode) {
            options.insert("access-mode".to_string(), v);
        }
        if toml_config.security.enable_2fa {
            options.insert("enable-2fa".to_string(), Self::map_bool(true));
        }
        if toml_config.security.whitelist_enabled {
            options.insert("whitelist-enabled".to_string(), Self::map_bool(true));
        }
        if !toml_config.security.whitelist.is_empty() {
            options.insert(
                "whitelist".to_string(),
                Self::map_list(&toml_config.security.whitelist),
            );
        }
        if let Some(v) = Self::map_optional_string(&toml_config.network.network_type) {
            options.insert("network-type".to_string(), v);
        }

        if let Some(v) = Self::map_optional_string(&toml_config.display.image_quality) {
            options.insert("image-quality".to_string(), v);
        }
        if let Some(v) = Self::map_optional_string(&toml_config.display.view_style) {
            options.insert("view-style".to_string(), v);
        }
        if let Some(v) = Self::map_optional_string(&toml_config.display.scroll_style) {
            options.insert("scroll-style".to_string(), v);
        }
        if toml_config.display.show_remote_cursor {
            options.insert("show-remote-cursor".to_string(), Self::map_bool(true));
        }
        if toml_config.display.disable_audio {
            options.insert("disable-audio".to_string(), Self::map_bool(true));
        }
        if toml_config.display.disable_clipboard {
            options.insert("disable-clipboard".to_string(), Self::map_bool(true));
        }

        for (k, v) in &toml_config.options {
            log::warn!("忽略未知配置项: {}", k);
            options.insert(k.clone(), v.clone());
        }

        let password = Self::map_optional_string(&toml_config.security.password);
        let rendezvous_server = Self::map_optional_string(&toml_config.rendezvous_server);

        let rendezvous_servers: Vec<ServerConfig> = toml_config
            .rendezvous_servers
            .iter()
            .map(to_server_config)
            .collect();

        let socks = if !toml_config.network.proxy.address.is_empty() {
            Some(Socks5Server {
                proxy: format!(
                    "{}:{}",
                    toml_config.network.proxy.address, toml_config.network.proxy.port
                ),
                username: toml_config.network.proxy.username,
                password: toml_config.network.proxy.password,
            })
        } else {
            None
        };

        MappedConfig {
            password,
            rendezvous_server,
            rendezvous_servers,
            socks,
            options,
        }
    }
}

fn to_server_config(entry: &TomlServerEntry) -> ServerConfig {
    let mut sc = ServerConfig::default();
    if !entry.id.is_empty() {
        sc.id = entry.id.clone();
    }
    sc.name = entry.name.clone();
    sc.id_server = entry.id_server.clone();
    if entry.id_port != 0 {
        sc.id_port = entry.id_port;
    }
    sc.relay_server = entry.relay_server.clone();
    if let Some(p) = entry.relay_port {
        sc.relay_port = Some(p);
    }
    sc.is_default = entry.is_default;
    sc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_bool() {
        assert_eq!(FieldMapper::map_bool(true), "Y");
        assert_eq!(FieldMapper::map_bool(false), "N");
    }

    #[test]
    fn test_map_list() {
        assert_eq!(FieldMapper::map_list(&[]), "");
        assert_eq!(FieldMapper::map_list(&["a".to_string()]), "a");
        assert_eq!(
            FieldMapper::map_list(&["a".to_string(), "b".to_string(), "c".to_string()]),
            "a,b,c"
        );
    }

    #[test]
    fn test_map_optional_string() {
        assert_eq!(FieldMapper::map_optional_string(""), None);
        assert_eq!(
            FieldMapper::map_optional_string("x"),
            Some("x".to_string())
        );
    }

    #[test]
    fn test_map_empty_config() {
        let mapped = FieldMapper::map_to_internal_config(TomlConfig::default());
        assert!(mapped.password.is_none());
        assert!(mapped.rendezvous_server.is_none());
        assert!(mapped.rendezvous_servers.is_empty());
        assert!(mapped.socks.is_none());
        assert!(mapped.options.is_empty());
    }

    #[test]
    fn test_map_full_config() {
        let content = r#"
rendezvous_server = "rs1.rustdesk.com"
relay_server = "relay.rustdesk.com"
api_server = "api.rustdesk.com"

[security]
password = "pw"
access_mode = "full"
enable_2fa = true
whitelist_enabled = true
whitelist = ["d1", "d2"]

[network]
network_type = "direct"

[network.proxy]
address = "127.0.0.1"
port = 1080
username = "u"
password = "p"

[display]
image_quality = "high"
show_remote_cursor = true
"#;
        let cfg: TomlConfig = hbb_common::toml::from_str(content).unwrap();
        let mapped = FieldMapper::map_to_internal_config(cfg);

        assert_eq!(mapped.password.as_deref(), Some("pw"));
        assert_eq!(mapped.rendezvous_server.as_deref(), Some("rs1.rustdesk.com"));
        assert_eq!(mapped.options.get("relay-server").unwrap(), "relay.rustdesk.com");
        assert_eq!(mapped.options.get("api-server").unwrap(), "api.rustdesk.com");
        assert_eq!(mapped.options.get("access-mode").unwrap(), "full");
        assert_eq!(mapped.options.get("enable-2fa").unwrap(), "Y");
        assert_eq!(mapped.options.get("whitelist-enabled").unwrap(), "Y");
        assert_eq!(mapped.options.get("whitelist").unwrap(), "d1,d2");
        assert_eq!(mapped.options.get("network-type").unwrap(), "direct");
        assert_eq!(mapped.options.get("image-quality").unwrap(), "high");
        assert_eq!(mapped.options.get("show-remote-cursor").unwrap(), "Y");
        let socks = mapped.socks.unwrap();
        assert_eq!(socks.proxy, "127.0.0.1:1080");
        assert_eq!(socks.username, "u");
        assert_eq!(socks.password, "p");
    }

    #[test]
    fn test_map_multi_servers() {
        let content = r#"
[[rendezvous_servers]]
name = "Primary"
id_server = "rs1.rustdesk.com"
id_port = 21116
relay_server = "relay1.rustdesk.com"
relay_port = 21117
is_default = true

[[rendezvous_servers]]
name = "Backup"
id_server = "rs2.rustdesk.com"
"#;
        let cfg: TomlConfig = hbb_common::toml::from_str(content).unwrap();
        let mapped = FieldMapper::map_to_internal_config(cfg);
        assert_eq!(mapped.rendezvous_servers.len(), 2);
        assert_eq!(mapped.rendezvous_servers[0].name, "Primary");
        assert_eq!(mapped.rendezvous_servers[0].id_server, "rs1.rustdesk.com");
        assert_eq!(mapped.rendezvous_servers[0].id_port, 21116);
        assert_eq!(
            mapped.rendezvous_servers[0].relay_server.as_deref(),
            Some("relay1.rustdesk.com")
        );
        assert!(mapped.rendezvous_servers[0].is_default);
        assert_eq!(mapped.rendezvous_servers[1].name, "Backup");
        assert_eq!(mapped.rendezvous_servers[1].id_server, "rs2.rustdesk.com");
        assert!(!mapped.rendezvous_servers[1].id.is_empty());
    }
}
