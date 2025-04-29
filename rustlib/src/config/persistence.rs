//! Atomically load, migrate and save configurations

use std::fs;
use std::fs::create_dir_all;
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use boringtun::x25519::StaticSecret;
use chrono::Utc;
use obscuravpn_api::cmd::ExitList;
use obscuravpn_api::types::{AccountId, WgPubkey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tempfile::{NamedTempFile, PersistError};
use thiserror::Error;
use x25519_dalek::PublicKey;

use crate::client_state::AccountStatus;
use crate::config::cached::ConfigCached;
use crate::manager::ExitSelector;

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
pub fn load(config_dir: &Path) -> Result<Config, ConfigLoadError> {
    let path = Path::new(config_dir).join(CONFIG_FILE);

    let err = match try_load(&path) {
        Ok(c) => return Ok(c.unwrap_or_default()),
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

    Ok(default_config)
}

pub fn save(config_dir: &Path, config: &Config) -> Result<(), ConfigSaveError> {
    let json = match serde_json::to_vec_pretty(config) {
        Ok(json) => json,
        Err(error) => {
            tracing::error!(
                    config.dir =? config_dir,
                    ?error,
                    "config::save could not encode config"
            );
            return Err(ConfigSaveError::SerializeError(error));
        }
    };

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

    if let Err(error) = file.as_file_mut().sync_data() {
        tracing::error!(
            config.dir =? config_dir,
            ?error,
            "config::save could not sync the temporary file"
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
/// - If the field's value needs to persist on logout, ensure so by updating `ClientState::set_account_id`
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[allow(clippy::manual_non_exhaustive)]
#[serde(default)]
pub struct Config {
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub api_url: Option<String>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub account_id: Option<AccountId>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub auto_connect: bool,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub old_account_ids: Vec<AccountId>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub local_tunnels_ids: Vec<String>,
    #[serde(skip)]
    pub exit: (), // Removed
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub in_new_account_flow: bool,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub cached_auth_token: Option<String>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub cached_exits: Option<ConfigCached<Arc<ExitList>>>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub pinned_locations: Vec<PinnedLocation>,

    // Deprecated, left in for migration only.
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub last_chosen_exit: Option<String>,

    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub last_chosen_exit_selector: ExitSelector,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub wireguard_key_cache: WireGuardKeyCache,
    #[serde(skip)]
    pub use_wireguard_key_cache: (), // Removed
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub cached_account_status: Option<AccountStatus>,
}

impl Config {
    pub fn migrate(&mut self) {
        if self.last_chosen_exit_selector == (ExitSelector::Any {}) {
            if let Some(exit) = &self.last_chosen_exit {
                self.last_chosen_exit_selector = ExitSelector::Exit { id: exit.clone() };
            }
        }
    }
}

// Redact sensitive fields by default
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigDebug {
    pub api_url: Option<String>,
    pub cached_exits: Option<ConfigCached<Arc<ExitList>>>,
    pub local_tunnels_ids: Vec<String>,
    pub in_new_account_flow: bool,
    pub pinned_locations: Vec<PinnedLocation>,
    pub last_chosen_exit: Option<String>,
    pub last_chosen_exit_selector: ExitSelector,
    pub use_wireguard_key_cache: (),
    pub has_account_id: bool,
    pub has_cached_auth_token: bool,
    pub auto_connect: bool,
}

impl From<Config> for ConfigDebug {
    fn from(config: Config) -> Self {
        let Config {
            api_url,
            account_id,
            old_account_ids: _,
            local_tunnels_ids,
            exit: (),
            in_new_account_flow,
            cached_auth_token,
            cached_exits,
            pinned_locations,
            last_chosen_exit,
            last_chosen_exit_selector,
            wireguard_key_cache: _,
            use_wireguard_key_cache,
            cached_account_status: _,
            auto_connect,
        } = config;
        Self {
            api_url,
            cached_exits,
            local_tunnels_ids,
            in_new_account_flow,
            pinned_locations,
            last_chosen_exit,
            last_chosen_exit_selector,
            use_wireguard_key_cache,
            has_account_id: account_id.is_some(),
            has_cached_auth_token: cached_auth_token.is_some(),
            auto_connect,
        }
    }
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
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct WireGuardKeyCache {
    #[serde_as(as = "serde_with::base64::Base64")]
    secret_key: [u8; 32],
    #[serde_as(as = "Option<serde_with::TimestampSeconds>")]
    first_use: Option<SystemTime>,
    #[serde_as(as = "Option<serde_with::TimestampSeconds>")]
    registered_at: Option<SystemTime>,
    #[serde_as(as = "Vec<serde_with::base64::Base64>")]
    old_public_keys: Vec<[u8; 32]>,
}

impl core::fmt::Debug for WireGuardKeyCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { secret_key, old_public_keys, first_use, registered_at } = self;
        let public_key = WgPubkey(PublicKey::from(&StaticSecret::from(*secret_key)).to_bytes());
        let old_public_keys: Vec<WgPubkey> = old_public_keys.iter().map(|b| WgPubkey(*b)).collect();
        f.debug_struct("WireGuardKeyCache")
            .field("public_key", &public_key)
            .field("first_use", first_use)
            .field("registered_at", registered_at)
            .field("old_public_keys", &old_public_keys)
            .finish()
    }
}

impl Default for WireGuardKeyCache {
    fn default() -> Self {
        let secret_key = if cfg!(test) {
            // deterministic value for serialization tests
            [1u8; 32]
        } else {
            StaticSecret::random_from_rng(OsRng).to_bytes()
        };
        Self { secret_key, first_use: None, registered_at: None, old_public_keys: Vec::new() }
    }
}

impl WireGuardKeyCache {
    pub fn use_key_pair(&mut self) -> (StaticSecret, PublicKey) {
        let secret_key = StaticSecret::from(self.secret_key);
        let public_key = PublicKey::from(&secret_key);
        let now = SystemTime::now();
        self.first_use.get_or_insert(now);
        (secret_key, public_key)
    }
    pub fn rotate_now(&mut self) {
        tracing::info!("rotating wireguard key pair");
        let mut old_public_keys = std::mem::take(&mut self.old_public_keys);
        old_public_keys.push(PublicKey::from(&StaticSecret::from(self.secret_key)).to_bytes());
        let secret_key = StaticSecret::random_from_rng(OsRng).to_bytes();
        *self = Self { secret_key, first_use: None, registered_at: None, old_public_keys }
    }
    pub fn rotate_now_if_not_recent(&mut self) {
        let first_used_or_registered_at = match (self.first_use, self.registered_at) {
            (Some(t1), Some(t2)) => Some(t1.min(t2)),
            (maybe_t1, maybe_t2) => maybe_t1.or(maybe_t2),
        };
        let not_recent = first_used_or_registered_at
            .and_then(|t| t.elapsed().ok())
            .map(|d| d > Duration::from_secs(60))
            .unwrap_or(true);
        if not_recent {
            self.rotate_now();
        }
    }
    pub fn rotate_if_required(&mut self) {
        const MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30); // 30 days
        if self.first_use.is_some_and(|t| t.elapsed().is_ok_and(|age| age > MAX_AGE)) {
            self.rotate_now();
        } else {
            tracing::info!("no wireguard key pair rotation required");
        }
    }
    pub fn need_registration(&mut self) -> Option<(PublicKey, Vec<PublicKey>)> {
        if self.registered_at.is_none() {
            let secret_key = StaticSecret::from(self.secret_key);
            let current_public_key = PublicKey::from(&secret_key);
            let old_public_keys = self.old_public_keys.iter().copied().map(Into::into).collect();
            return Some((current_public_key, old_public_keys));
        }
        None
    }
    pub fn registered(&mut self, removed_public_keys: &[PublicKey]) {
        self.registered_at = Some(SystemTime::now());
        self.old_public_keys.retain(|b| !removed_public_keys.contains(&PublicKey::from(*b)));
    }
}
