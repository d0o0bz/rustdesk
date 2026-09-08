use std::path::Path;

use hbb_common::config::{
    Config, ConfigValidator, MultiServerStore, ServerConfig, ServerConfigRepository,
    MAX_SERVER_CONFIGS,
};

use super::error::{ConfigImportError, TomlParseError};
use super::field_mapper::{FieldMapper, MappedConfig};
use super::toml_config::{TomlConfig, SUPPORTED_VERSION};
use super::toml_parser::TomlConfigParser;

pub struct ConfigImporter;

/// What an import produced that the caller still has to pass on.
#[derive(Debug, Default)]
pub struct ImportOutcome {
    /// The serialized store to hand to the other processes, `None` when nothing was published.
    ///
    /// Publishing only reached this process' own copy of the options. Everything else learns
    /// the list through ipc, which on Windows is the only channel there is, and that step is
    /// the caller's: nothing here can reach it.
    pub published: Option<String>,
}

impl ConfigImporter {
    pub fn import_from_path(path: &Path) -> Result<ImportOutcome, ConfigImportError> {
        if !path.is_file() {
            return Err(ConfigImportError::TomlConfigNotFound);
        }
        super::path_safety::validate_path_safety(path)?;
        if already_imported(path) {
            log::info!("配置文件与上次导入一致，跳过导入");
            return Ok(ImportOutcome::default());
        }
        let toml_config = TomlConfigParser::parse(path)?;
        if let Some(found) = toml_config.unsupported_version() {
            // Every field here is optional, so a layout from another release reads as a file
            // asking for nothing and would leave the machine silently unconfigured.
            return Err(ConfigImportError::TomlParseError(
                TomlParseError::UnsupportedVersion {
                    found: found.to_string(),
                    supported: SUPPORTED_VERSION.to_string(),
                },
            ));
        }
        if toml_config.is_empty() {
            log::info!("空 TOML 配置，跳过导入");
            return Ok(ImportOutcome::default());
        }
        let mapped = FieldMapper::map_to_internal_config(toml_config);
        check_incoming_servers(&mapped.rendezvous_servers)?;
        let outcome = Self::merge_and_store(&mapped)?;
        record_import_source(path);
        Ok(outcome)
    }

    fn merge_and_store(mapped: &MappedConfig) -> Result<ImportOutcome, ConfigImportError> {
        let mut published = None;
        if let Some(pw) = &mapped.password {
            if !Config::set_permanent_password(pw) {
                // Changing it is either disabled by policy or the value could not be prepared
                // for storage. Either way the file asked for this password and the machine does
                // not have it, which a deployment has to be able to tell from a success.
                return Err(ConfigImportError::PermissionDenied(
                    "永久密码设置失败，可能已被策略禁用".to_string(),
                ));
            }
        }
        // Extended options first. The top level `rendezvous_server` names the very option an
        // `[options]` entry can too, and the one spelled out for it has to win rather than
        // whichever of the two is written last.
        for (k, v) in &mapped.options {
            Config::set_option(k.clone(), v.clone());
        }
        if let Some(server) = &mapped.rendezvous_server {
            Config::set_option("custom-rendezvous-server".to_string(), server.clone());
        }
        if let Some(socks) = &mapped.socks {
            Config::set_socks(Some(socks.clone()));
        }
        if !mapped.rendezvous_servers.is_empty() {
            // Read after the option writes above: a top level `rendezvous_server` has already
            // been mirrored into the store by the sync hook, so this says whether the single
            // server settings are authoritative and must not be overridden.
            //
            // The stored value rather than the effective one. A build that presets the server
            // answers that preset for every question about it, and the list could then never
            // elect a default of its own.
            let single_server_empty = Config::get_stored_option("custom-rendezvous-server")
                .map_or(true, |v| v.is_empty());
            let mut store = MultiServerStore::load();
            let default_config =
                merge_servers(&mut store, &mapped.rendezvous_servers, single_server_empty);
            // Persist before applying, so the sync hook the apply triggers finds this entry by
            // `id_server` and updates it instead of adding another one. A store that never
            // reached the disk leaves the list on whatever was there before, which is not
            // something to apply a default from and report as imported.
            store
                .try_save()
                .map_err(|e| ConfigImportError::StoreError(format!("服务器配置保存失败: {e}")))?;
            if let Some(mut config) = default_config {
                log::info!(
                    "导入服务器配置默认项: {} ({})",
                    config.name,
                    config.id_server
                );
                if single_server_empty {
                    // `apply_current` writes every server option and reads a missing relay or
                    // api as none, so an entry silent about them would drop what the same file
                    // asked for higher up, or what was configured before this import.
                    let relay = Config::get_option("relay-server");
                    if config.relay_server.is_none() && !relay.is_empty() {
                        config.relay_server = Some(relay);
                    }
                    let api = Config::get_option("api-server");
                    if config.api_server.is_none() && !api.is_empty() {
                        config.api_server = Some(api);
                    }
                    // Only the options drive the connection, so a default recorded in the store
                    // alone would leave it on whatever server was in use before.
                    ServerConfigRepository::apply_current(&config, &mut |k, v| {
                        Config::set_option(k, v)
                    });
                    ServerConfigRepository::save_current(&config.id)
                        .map_err(|e| ConfigImportError::StoreError(e.to_string()))?;
                }
            }
            // Hand the result to every process. `store.save` alone is not enough: it stays
            // quiet while the shared option is still empty, and reading back through
            // `MultiServerStore::load` would hand over that option rather than the list just
            // written, so the service would stay on its own single entry copy and the ui would
            // keep showing a list an earlier publish had shadowed.
            published = MultiServerStore::publish_from_file_if_owner();
        }
        Ok(ImportOutcome { published })
    }
}

/// Fold `servers` into `store`, keeping one entry per server and electing a default.
///
/// Matching falls back to `id_server` because the sync hook builds its entry with a generated
/// id, so an import that also sets the single server options would otherwise append a duplicate.
/// Returns the elected default so the caller can persist first and then put it in use.
fn merge_servers(
    store: &mut MultiServerStore,
    servers: &[ServerConfig],
    single_server_empty: bool,
) -> Option<ServerConfig> {
    let mut resolved = Vec::with_capacity(servers.len());
    for new_sc in servers {
        let idx = store
            .rendezvous_servers
            .iter()
            .position(|c| c.id == new_sc.id)
            .or_else(|| {
                store
                    .rendezvous_servers
                    .iter()
                    .position(|c| !new_sc.id_server.is_empty() && c.id_server == new_sc.id_server)
            });
        let pos = match idx {
            Some(i) => {
                // Keep the stored id, otherwise the uuid generated per import would orphan
                // `current_config_id` and any default marker pointing at this server.
                let kept_id = store.rendezvous_servers[i].id.clone();
                let mut updated = new_sc.clone();
                updated.id = kept_id;
                overlay_server(&mut store.rendezvous_servers[i], &updated);
                i
            }
            None => {
                store.rendezvous_servers.push(new_sc.clone());
                store.rendezvous_servers.len() - 1
            }
        };
        resolved.push(pos);
    }

    let default_pos = if servers.iter().any(|c| c.is_default) {
        resolved
            .iter()
            .copied()
            .find(|&i| store.rendezvous_servers[i].is_default)
    } else if single_server_empty {
        resolved.first().copied()
    } else {
        None
    }?;
    // Upstream keeps exactly one default, so demote the others.
    for (i, c) in store.rendezvous_servers.iter_mut().enumerate() {
        c.is_default = i == default_pos;
    }
    Some(store.rendezvous_servers[default_pos].clone())
}

/// Overlay `new` onto `stored`, leaving alone whatever the file stayed silent about.
///
/// Replacing the entry outright read every omitted field as deliberately empty, so importing
/// the same servers twice wiped `api_server` and `key`, and electing the result default then
/// wrote those blanks over the single server options.
fn overlay_server(stored: &mut ServerConfig, new: &ServerConfig) {
    stored.name = new.name.clone();
    stored.id_server = new.id_server.clone();
    stored.id_port = new.id_port;
    stored.is_default = new.is_default;
    if new.relay_server.is_some() {
        stored.relay_server = new.relay_server.clone();
    }
    if new.relay_port.is_some() {
        stored.relay_port = new.relay_port;
    }
    if new.api_server.is_some() {
        stored.api_server = new.api_server.clone();
    }
    if new.key.is_some() {
        stored.key = new.key.clone();
    }
}

/// Where the last successful import read its source from.
///
/// Comparing mtimes cannot answer "already imported": unless the import also sets a password
/// it never touches the file [`Config::file`] points at, so that side of the comparison is
/// stuck at whenever it happened to be written and either never becomes newer - every run
/// redoes the work - or already is, which skips a redeployed source forever because the file
/// kept its packaging timestamp. Recording what the import itself has done has no such hole.
const OPTION_IMPORT_SOURCE: &str = "toml-import-source";

/// Identity of the source file: path, size and mtime, `None` when it cannot be read.
fn source_fingerprint(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(format!(
        "{}|{}|{}",
        path.to_string_lossy(),
        meta.len(),
        mtime
    ))
}

fn already_imported(path: &Path) -> bool {
    let Some(fingerprint) = source_fingerprint(path) else {
        return false;
    };
    Config::get_stored_option(OPTION_IMPORT_SOURCE).as_deref() == Some(fingerprint.as_str())
}

fn record_import_source(path: &Path) {
    if let Some(fingerprint) = source_fingerprint(path) {
        Config::set_option(OPTION_IMPORT_SOURCE.to_owned(), fingerprint);
    }
}

/// Refuse a server list the store cannot take, before anything has been written for it.
///
/// Writing first and checking afterwards would leave a refused file half applied, so this has
/// to stand on its own.
fn check_incoming_servers(servers: &[ServerConfig]) -> Result<(), ConfigImportError> {
    if servers.is_empty() {
        return Ok(());
    }
    check_incoming_entries(servers)?;
    check_incoming_capacity(servers)
}

/// Hold every entry to what the rest of the app already requires.
///
/// The import used to skip these checks altogether, so an out of range port reached the very
/// options the connection is built from, and an entry with nothing but a port got stored and
/// then elected default, leaving `custom-rendezvous-server` pointing at `":21116"`.
fn check_incoming_entries(servers: &[ServerConfig]) -> Result<(), ConfigImportError> {
    let mut ids = std::collections::HashSet::new();
    let mut id_servers = std::collections::HashSet::new();
    for s in servers {
        ConfigValidator::validate_format(s).map_err(|e| {
            ConfigImportError::InvalidServerConfig(format!("{} ({})", e, s.id_server))
        })?;
        // Two entries claiming one identity do not both survive the merge: the second matches
        // and overwrites the first, silently dropping a server the file asked for.
        if !ids.insert(s.id.as_str()) {
            return Err(ConfigImportError::InvalidServerConfig(format!(
                "重复的服务器标识 {}",
                s.id
            )));
        }
        if !id_servers.insert(s.id_server.as_str()) {
            return Err(ConfigImportError::InvalidServerConfig(format!(
                "重复的 ID 服务器 {}",
                s.id_server
            )));
        }
    }
    Ok(())
}

/// Keep the list within what anything downstream can present.
///
/// Past [`MAX_SERVER_CONFIGS`] entries live on disk where nothing shows them and the user
/// cannot remove them, and the sync hook stops mirroring the server in use at all. Only an
/// addition can breach the cap: refusing to update what is already stored would lock the file
/// out for good on a store left over from an earlier version.
fn check_incoming_capacity(servers: &[ServerConfig]) -> Result<(), ConfigImportError> {
    let store = MultiServerStore::load();
    let adding = servers
        .iter()
        .filter(|s| {
            !store
                .rendezvous_servers
                .iter()
                .any(|c| c.id == s.id || (!c.id_server.is_empty() && c.id_server == s.id_server))
        })
        .count();
    if adding > 0 && store.rendezvous_servers.len() + adding > MAX_SERVER_CONFIGS {
        return Err(ConfigImportError::InvalidServerConfig(format!(
            "服务器数量超过上限 {}（已有 {}，新增 {}）",
            MAX_SERVER_CONFIGS,
            store.rendezvous_servers.len(),
            adding
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_nonexistent_path() {
        let path = std::path::Path::new("/nonexistent/rustdesk_test_import.toml");
        let res = ConfigImporter::import_from_path(path);
        assert!(matches!(res, Err(ConfigImportError::TomlConfigNotFound)));
    }

    #[test]
    fn test_source_fingerprint_tracks_file_identity() {
        let mut path = std::env::temp_dir();
        path.push(format!("rustdesk_import_fp_{}.toml", std::process::id()));
        std::fs::write(&path, b"rendezvous_server = \"a\"\n").unwrap();
        let first = source_fingerprint(&path).unwrap();
        assert_eq!(source_fingerprint(&path).as_deref(), Some(first.as_str()));
        std::fs::write(&path, b"rendezvous_server = \"ab\"\n").unwrap();
        assert_ne!(
            source_fingerprint(&path).as_deref(),
            Some(first.as_str()),
            "a source rewritten after an import is not the one already imported"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_check_incoming_entries_rejects_out_of_range_port() {
        let mut s = server("a", "10.0.0.1", false);
        s.id_port = 70000;
        assert!(matches!(
            check_incoming_entries(std::slice::from_ref(&s)),
            Err(ConfigImportError::InvalidServerConfig(_))
        ));
    }

    #[test]
    fn test_check_incoming_entries_rejects_duplicate_id_server() {
        // Two entries for one identity do not both survive: the later one matches and
        // overwrites the earlier, silently dropping a server the file asked for.
        let servers = vec![
            server("a", "10.0.0.1", false),
            server("b", "10.0.0.1", false),
        ];
        assert!(matches!(
            check_incoming_entries(&servers),
            Err(ConfigImportError::InvalidServerConfig(_))
        ));
    }

    #[test]
    fn test_check_incoming_entries_rejects_entry_without_a_name() {
        let mut s = server("a", "10.0.0.1", false);
        s.name = String::new();
        assert!(matches!(
            check_incoming_entries(std::slice::from_ref(&s)),
            Err(ConfigImportError::InvalidServerConfig(_))
        ));
    }

    #[test]
    fn test_check_incoming_entries_accepts_distinct_servers() {
        let servers = vec![
            server("a", "10.0.0.1", false),
            server("b", "10.0.0.2", true),
        ];
        assert!(check_incoming_entries(&servers).is_ok());
    }

    fn server(id: &str, id_server: &str, is_default: bool) -> ServerConfig {
        ServerConfig {
            id: id.to_string(),
            name: id_server.to_string(),
            id_server: id_server.to_string(),
            is_default,
            ..Default::default()
        }
    }

    #[test]
    fn test_merge_servers_keeps_stored_fields_the_file_omits() {
        // Importing the same servers twice must not clear what the first run stored: silence
        // about a field is not a request to empty it.
        let mut stored = server("sync-uuid", "10.0.0.1", true);
        stored.api_server = Some("api.example.com".to_string());
        stored.key = Some("pubkey".to_string());
        let mut store = MultiServerStore {
            rendezvous_servers: vec![stored],
            current_config_id: Some("sync-uuid".to_string()),
        };
        merge_servers(
            &mut store,
            &[server("import-uuid", "10.0.0.1", false)],
            false,
        );
        assert_eq!(store.rendezvous_servers.len(), 1);
        assert_eq!(store.rendezvous_servers[0].id, "sync-uuid");
        assert_eq!(
            store.rendezvous_servers[0].api_server.as_deref(),
            Some("api.example.com")
        );
        assert_eq!(store.rendezvous_servers[0].key.as_deref(), Some("pubkey"));
    }

    #[test]
    fn test_merge_servers_applies_fields_the_file_gives() {
        let mut store = MultiServerStore {
            rendezvous_servers: vec![server("sync-uuid", "10.0.0.1", true)],
            current_config_id: Some("sync-uuid".to_string()),
        };
        let mut wanted = server("import-uuid", "10.0.0.1", false);
        wanted.key = Some("rotated".to_string());
        merge_servers(&mut store, &[wanted], false);
        assert_eq!(store.rendezvous_servers[0].key.as_deref(), Some("rotated"));
    }

    #[test]
    fn test_merge_servers_dedupes_by_id_server() {
        // The sync hook builds its entry with a generated id, so an import naming the same
        // id_server has to update that entry instead of appending a second one.
        let mut store = MultiServerStore {
            rendezvous_servers: vec![server("sync-uuid", "10.0.0.1", true)],
            current_config_id: Some("sync-uuid".to_string()),
        };
        merge_servers(
            &mut store,
            &[server("import-uuid", "10.0.0.1", false)],
            false,
        );
        assert_eq!(store.rendezvous_servers.len(), 1);
        assert_eq!(store.rendezvous_servers[0].id, "sync-uuid");
    }

    #[test]
    fn test_merge_servers_appends_unknown_server() {
        let mut store = MultiServerStore::default();
        merge_servers(&mut store, &[server("a", "10.0.0.1", false)], false);
        assert_eq!(store.rendezvous_servers.len(), 1);
    }

    #[test]
    fn test_merge_servers_elects_first_when_single_server_empty() {
        let mut store = MultiServerStore {
            rendezvous_servers: vec![server("stale", "10.0.0.9", true)],
            current_config_id: None,
        };
        let imported = vec![
            server("a", "10.0.0.1", false),
            server("b", "10.0.0.2", false),
        ];
        let elected = merge_servers(&mut store, &imported, true);
        assert_eq!(
            elected.map(|c| c.id_server),
            Some("10.0.0.1".to_string()),
            "no entry asks to be the default, so the first one wins"
        );
        let defaults: Vec<_> = store
            .rendezvous_servers
            .iter()
            .filter(|c| c.is_default)
            .collect();
        assert_eq!(defaults.len(), 1, "the previous default must be demoted");
        assert_eq!(defaults[0].id, "a");
    }

    #[test]
    fn test_merge_servers_honours_explicit_default() {
        let mut store = MultiServerStore::default();
        let imported = vec![
            server("a", "10.0.0.1", false),
            server("b", "10.0.0.2", true),
        ];
        let elected = merge_servers(&mut store, &imported, true);
        assert_eq!(elected.map(|c| c.id_server), Some("10.0.0.2".to_string()));
    }

    #[test]
    fn test_merge_servers_leaves_default_when_single_server_set() {
        let mut store = MultiServerStore::default();
        let imported = vec![server("a", "10.0.0.1", false)];
        assert!(
            merge_servers(&mut store, &imported, false).is_none(),
            "the single server settings are authoritative, so no default is elected"
        );
        assert!(!store.rendezvous_servers[0].is_default);
    }
}
