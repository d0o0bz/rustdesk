use std::path::Path;

use hbb_common::config::{Config, MultiServerStore, ServerConfig, ServerConfigRepository};

use super::error::ConfigImportError;
use super::field_mapper::{FieldMapper, MappedConfig};
use super::toml_config::TomlConfig;
use super::toml_parser::TomlConfigParser;

pub struct ConfigImporter;

impl ConfigImporter {
    pub fn import_from_path(path: &Path) -> Result<(), ConfigImportError> {
        if !path.is_file() {
            return Err(ConfigImportError::TomlConfigNotFound);
        }
        super::path_safety::validate_path_safety(path)?;
        if is_source_stale(path) {
            log::info!("配置文件未更新，跳过导入");
            return Ok(());
        }
        let toml_config = TomlConfigParser::parse(path)?;
        if toml_config.is_empty() {
            log::info!("空 TOML 配置，跳过导入");
            return Ok(());
        }
        let mapped = FieldMapper::map_to_internal_config(toml_config);
        Self::merge_and_store(&mapped)?;
        Ok(())
    }

    fn merge_and_store(mapped: &MappedConfig) -> Result<(), ConfigImportError> {
        if let Some(pw) = &mapped.password {
            if !Config::set_permanent_password(pw) {
                log::warn!("永久密码设置失败");
            }
        }
        if let Some(server) = &mapped.rendezvous_server {
            Config::set_option("custom-rendezvous-server".to_string(), server.clone());
        }
        if let Some(socks) = &mapped.socks {
            Config::set_socks(Some(socks.clone()));
        }
        for (k, v) in &mapped.options {
            Config::set_option(k.clone(), v.clone());
        }
        if !mapped.rendezvous_servers.is_empty() {
            // Read after the option writes above: a top level `rendezvous_server` has already
            // been mirrored into the store by the sync hook, so this says whether the single
            // server settings are authoritative and must not be overridden.
            let single_server_empty = Config::get_option("custom-rendezvous-server").is_empty();
            let mut store = MultiServerStore::load();
            let default_config =
                merge_servers(&mut store, &mapped.rendezvous_servers, single_server_empty);
            // Persist before applying, so the sync hook the apply triggers finds this entry by
            // `id_server` and updates it instead of adding another one.
            store.save();
            if let Some(config) = default_config {
                log::info!(
                    "导入服务器配置默认项: {} ({})",
                    config.name,
                    config.id_server
                );
                if single_server_empty {
                    // Only the options drive the connection, so a default recorded in the store
                    // alone would leave it on whatever server was in use before.
                    ServerConfigRepository::apply_current(&config, &mut |k, v| {
                        Config::set_option(k, v)
                    });
                    if let Err(e) = ServerConfigRepository::save_current(&config.id) {
                        log::warn!("当前服务器配置保存失败: {}", e);
                    }
                }
            }
            // Hand the result to every process. `store.save` alone is not enough: it stays
            // quiet while the shared option is still empty, and reading back through
            // `MultiServerStore::load` would hand over that option rather than the list just
            // written, so the service would stay on its own single entry copy and the ui would
            // keep showing a list an earlier publish had shadowed.
            let _ = MultiServerStore::publish_from_file_if_owner();
        }
        Ok(())
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
                store.rendezvous_servers[i] = updated;
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

fn is_source_stale(path: &Path) -> bool {
    let src_mtime = hbb_common::get_modified_time(path);
    let cfg_mtime = hbb_common::get_modified_time(&Config::file());
    src_mtime <= cfg_mtime
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
