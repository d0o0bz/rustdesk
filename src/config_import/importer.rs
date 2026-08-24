use std::path::Path;

use hbb_common::config::{Config, Config2};

use super::error::ConfigImportError;
use super::field_mapper::{FieldMapper, MappedConfig};
use super::install_dir::InstallDirDetector;
use super::toml_config::TomlConfig;
use super::toml_parser::TomlConfigParser;

pub struct ConfigImporter;

impl ConfigImporter {
    pub fn import_from_path(path: &Path) -> Result<(), ConfigImportError> {
        if !path.is_file() {
            return Err(ConfigImportError::TomlConfigNotFound);
        }
        super::install_dir::validate_path_safety(path)?;
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

    pub fn import_from_install_dir() -> Result<(), ConfigImportError> {
        match InstallDirDetector::detect_toml_config() {
            Some(path) => {
                log::info!("检测到 TOML 配置: {:?}", path);
                Self::import_from_path(&path)
            }
            None => {
                log::info!("未检测到 TOML 配置文件");
                Ok(())
            }
        }
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
            let mut config2 = Config2::get();
            for new_sc in &mapped.rendezvous_servers {
                if let Some(existing) = config2
                    .rendezvous_servers
                    .iter_mut()
                    .find(|c| c.id == new_sc.id)
                {
                    *existing = new_sc.clone();
                } else {
                    config2.rendezvous_servers.push(new_sc.clone());
                }
            }
            Config2::set(config2);
        }
        Ok(())
    }
}

fn is_source_stale(path: &Path) -> bool {
    let src_mtime = hbb_common::get_modified_time(path);
    let cfg_mtime = hbb_common::get_modified_time(&Config::file());
    let exe_mtime = hbb_common::get_exe_time();
    !(src_mtime > cfg_mtime && src_mtime < exe_mtime)
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
    fn test_import_from_install_dir_no_panic() {
        let _ = ConfigImporter::import_from_install_dir();
    }
}
