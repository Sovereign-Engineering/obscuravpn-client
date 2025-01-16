//! Atomically load, migrate and save configurations

use std::fs;
use std::fs::create_dir_all;
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::time::SystemTime;

use boringtun::x25519::StaticSecret;
use chrono::Utc;
use obscuravpn_api::types::WgPubkey;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use tempfile::{NamedTempFile, PersistError};
use thiserror::Error;
use x25519_dalek::PublicKey;

pub(super) const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Error)]
pub enum ConfigSaveError {
    #[error("could not serialize config: {0}")]
    SerializeError(serde_json::Error),
    #[error("could not create directory: {0}")]
    CreateDirError(std::io::Error),
    #[error("could not create temporary file: {0}")]
    CreateTempFileError(std::io::Error),
    #[error("could not write to temporary file: {0}")]
    TempFileWriteError(std::io::Error),
    #[error("could not persist temporary file: {0}")]
    TempFilePersistError(PersistError),
}

#[derive(Debug, Error)]
pub enum ConfigLoadError {
    #[error("could not read config: {0}")]
    ReadError(std::io::Error),
    #[error("could not deserialize config: {0}")]
    DeserializeError(serde_json::Error),
    #[error("config not save config: {0}")]
    SaveEror(ConfigSaveError),
}

fn try_load(path: &Path) -> Result<Option<Config>, ConfigLoadError> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(err) => {
            if err.kind() == ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(ConfigLoadError::ReadError(err));
        }
    };

    match serde_json::from_reader(file) {
        Ok(c) => Ok(Some(c)),
        Err(err) => Err(ConfigLoadError::DeserializeError(err)),
    }
}

/// Load a single config path.
///
/// TODO: Remove after some migration period.
pub(in crate::config) fn load_one(config_dir: &Path) -> Result<Option<Config>, ConfigLoadError> {
    let path = Path::new(config_dir).join(CONFIG_FILE);

    let err = match try_load(&path) {
        Ok(c) => return Ok(c),
        Err(error) => {
            tracing::error!(
                config.path =? path,
                ?error,
                "Failed to load config, resetting.");
            error
        }
    };

    // This may collide if failing in a tight loop, that is fine. Possibly even a feature.
    let backup_path = Path::new(config_dir).join(format!("config-backup-{}.json", Utc::now().to_rfc3339()));

    // TODO: Do we want to try to clean up old backup configs?

    match fs::rename(&path, &backup_path) {
        Ok(()) => {}
        Err(error) => {
            tracing::error!(
                config.path =? path,
                config.backup_path =? backup_path,
                ?error,
                "Failed to move broken config.");
            return Err(err);
        }
    }

    let default_config = Default::default();

    // Ensure that we can write the config. Otherwise we may just crash when the user logs in if the disk is full or some other endemic issue.
    save(config_dir, &default_config).map_err(ConfigLoadError::SaveEror)?;

    Ok(Some(default_config))
}

/// Load and migrate a configuration.
pub fn load(config_dir: &Path, old_config_dir: &Path) -> Result<Config, ConfigLoadError> {
    // Note: We don't continue down the list on errors to avoid reading a potentially old config file (for example a new version stops writing the old one then the user rolls back). It also makes the whole process more predictable. We only fall back if the new location does not exist.
    if let Some(c) = load_one(config_dir)? {
        return Ok(c);
    }

    if let Some(c) = load_one(old_config_dir)? {
        tracing::info!(
            config.dir =? config_dir,
            config.old_dir =? old_config_dir,
            "Migrating config file to new location"
        );

        save(config_dir, &c).map_err(ConfigLoadError::SaveEror)?;

        tracing::info!(
            config.dir =? config_dir,
            config.old_dir =? old_config_dir,
            "Config migrated successfully"
        );

        // NOTE: We leave the old config in place. This is mostly because the app doesn't yet have a logout button so it is very difficult for the user to enter valuable data (their account ID) if we leave them logged in.

        return Ok(c);
    }

    Ok(Default::default())
}

pub fn save(config_dir: &Path, config: &Config) -> Result<(), ConfigSaveError> {
    let json = serde_json::to_vec_pretty(config).unwrap();

    if let Err(error) = create_dir_all(config_dir) {
        tracing::error!(
                config.dir =? config_dir,
                ?error,
                "config::save could not create config directory"
        );
        return Err(ConfigSaveError::CreateDirError(error));
    }

    let mut file = match NamedTempFile::new_in(config_dir) {
        Ok(f) => f,
        Err(error) => {
            tracing::error!(
                    config.dir =? config_dir,
                    ?error,
                    "config::save could not create temporary file"
            );
            return Err(ConfigSaveError::CreateTempFileError(error));
        }
    };

    if let Err(error) = file.write_all(&json).and_then(|_| file.flush()) {
        tracing::error!(
            config.dir =? config_dir,
            ?error,
            "config::save could not write to temporary file"
        );
        return Err(ConfigSaveError::TempFileWriteError(error));
    }

    let path = config_dir.join(CONFIG_FILE);
    if let Err(error) = file.persist(path) {
        tracing::error!(
            config.dir =? config_dir,
            ?error,
            "config::save could not persist the temporary file"
        );
        return Err(ConfigSaveError::TempFilePersistError(error));
    }

    Ok(())
}

/// This is the configuration structure as stored to disk.
///
/// TL;DR is that you must consider both forwards and backwards compatibility when modifying this type.
///
/// Please follow these rules:
/// - Never remove a field, instead remove `pub`, change the type to `()` and add `#[serde(skip)]`.
/// - Never change a field type. Add a new field with a different name instead.
///     - Consider reading and writing both fields for a few releases to preserve data on rollback.
/// - Fields must never fail to parse. The best way to do this is the `#[serde(deserialize_with = "crate::serde_safe::deserialize")]` attribute which resets to `Default` if the fields fails to parse.
///     - If the field value is complex you may want to make the field support partial parse failure internally as well.
/// - The more important some data is the simpler its type should be. Consider breaking important data out of complex types into simple top-level ones to reduce the risk of it getting reset to the default value..
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[allow(clippy::manual_non_exhaustive)]
#[serde(default)]
pub struct Config {
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub api_url: Option<String>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub account_id: Option<String>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub old_account_ids: Vec<String>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub local_tunnels_ids: Vec<String>,
    #[serde(skip)]
    pub exit: (), // Removed
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub in_new_account_flow: bool,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub cached_auth_token: Option<String>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub pinned_exits: Vec<String>,

    // Note: This is optional just for the migration. After migration we can make the default an empty list.
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub pinned_locations: Option<Vec<PinnedLocation>>,

    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub last_chosen_exit: Option<String>,

    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub wireguard_key_cache: WireGuardKeyCache,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub use_wireguard_key_cache: bool,
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PinnedLocation {
    pub country_code: String,
    pub city_code: String,

    #[serde_as(as = "serde_with::TimestampSeconds")]
    pub pinned_at: SystemTime,
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Clone)]
pub struct WireGuardKeyCache {
    pub secret_key: StaticSecret,
    #[serde_as(as = "serde_with::TimestampSeconds")]
    pub generated_at: SystemTime,
}

impl PartialEq for WireGuardKeyCache {
    fn eq(&self, other: &Self) -> bool {
        self.secret_key.as_bytes() == other.secret_key.as_bytes() && self.generated_at == other.generated_at
    }
}

impl Eq for WireGuardKeyCache {}

impl core::fmt::Debug for WireGuardKeyCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { secret_key, generated_at } = self;
        let public_key = WgPubkey(PublicKey::from(secret_key).to_bytes());
        f.debug_struct("WireGuardKeyCache")
            .field("public_key", &public_key)
            .field("cached_at", generated_at)
            .finish()
    }
}

impl Default for WireGuardKeyCache {
    fn default() -> Self {
        if cfg!(test) {
            // deterministic values for serialization tests
            return Self { secret_key: StaticSecret::from([1; 32]), generated_at: SystemTime::UNIX_EPOCH };
        }
        Self { secret_key: StaticSecret::random_from_rng(OsRng), generated_at: SystemTime::now() }
    }
}

impl WireGuardKeyCache {
    pub fn rotate_now(&mut self) {
        *self = Self::default();
    }
}
