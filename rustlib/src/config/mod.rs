mod persistence;

pub mod cached;
mod dns_cache;
pub mod feature_flags;
#[cfg(test)]
mod persistence_test;

use crate::errors::ConfigDirty;
pub use persistence::*;
use std::path::PathBuf;

pub struct ConfigHandle {
    config_dir: PathBuf,
    config: Config,
    dirty: bool,
}

impl ConfigHandle {
    pub fn new(config_dir: PathBuf, keychain_wg_sk: Option<&[u8]>) -> Result<Self, ConfigLoadError> {
        let mut config = load(&config_dir, keychain_wg_sk)?;
        config.migrate();
        Ok(Self { dirty: false, config_dir, config })
    }
    pub fn change<T>(&mut self, f: impl FnOnce(&mut Config) -> T) -> T {
        let mut new_config = self.config.clone();
        let ret = f(&mut new_config);
        self.dirty |= self.config != new_config;
        if self.dirty {
            // Config save errors are usually not recoverable and don't influence desired behavior, so we try, log and move on without returning the error
            match save(&self.config_dir, &new_config) {
                Ok(_) => self.dirty = false,
                Err(error) => tracing::error!(message_id = "C4t7uMUX", ?error, "error saving config: {error}"),
            }
        }
        self.config = new_config;
        ret
    }

    pub fn check_persisted(&self) -> Result<(), ConfigDirty> {
        (!self.dirty).then_some(()).ok_or(ConfigDirty)
    }
}

impl std::ops::Deref for ConfigHandle {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}
