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
use crate::config::feature_flags::FeatureFlags;
use crate::exit_selection::ExitSelector;

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
pub fn load(config_dir: &Path, keychain_wg_sk: Option<&[u8]>) -> Result<Config, ConfigLoadError> {
    let path = Path::new(config_dir).join(CONFIG_FILE);

    let err = match try_load(&path) {
        Ok(config) => {
            let mut config = config.unwrap_or_default();
            config.wireguard_key_cache.try_set_secret_key_from_keychain(keychain_wg_sk);
            return Ok(config);
        }
        Err(error) => match error {
            ConfigLoadError::ReadError(_) => return Err(error),
            ConfigLoadError::DeserializeError(_) => {
                tracing::error!(
                    ?error,
                    config.path =? path,
                    message_id = "Voosh7sa",
                    "Failed to parse config, resetting.");
                error
            }
            ConfigLoadError::SaveEror(_) => {
                tracing::warn!(
                    ?error,
                    config.path =? path,
                    message_id = "Voosh7sa",
                    "Reading config file returned save error.");
                return Err(error);
            }
        },
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
    let config = config.clone();
    let json = match serde_json::to_vec_pretty(&config) {
        Ok(json) => json,
        Err(error) => {
            tracing::error!(
                ?error,
                config.dir =? config_dir,
                message_id = "Chuzoe3k",
                "config::save could not encode config"
            );
            return Err(ConfigSaveError::SerializeError(error));
        }
    };

    if let Err(error) = create_dir_all(config_dir) {
        tracing::error!(
                ?error,
                config.dir =? config_dir,
                message_id = "kohLaih0",
                "config::save could not create config directory"
        );
        return Err(ConfigSaveError::CreateDirError(error));
    }

    let mut file = match NamedTempFile::new_in(config_dir) {
        Ok(f) => f,
        Err(error) => {
            tracing::error!(
                ?error,
                config.dir =? config_dir,
                message_id = "oPie5quu",
                "config::save could not create temporary file"
            );
            return Err(ConfigSaveError::CreateTempFileError(error));
        }
    };

    if let Err(error) = file.write_all(&json).and_then(|_| file.flush()) {
        tracing::error!(
            ?error,
            config.dir =? config_dir,
            message_id = "Ua7oosei",
            "config::save could not write to temporary file"
        );
        return Err(ConfigSaveError::TempFileWriteError(error));
    }

    if let Err(error) = file.as_file_mut().sync_data() {
        tracing::error!(
            ?error,
            config.dir =? config_dir,
            message_id = "Mahd5hei",
            "config::save could not sync the temporary file"
        );
        return Err(ConfigSaveError::TempFileWriteError(error));
    }

    let path = config_dir.join(CONFIG_FILE);
    if let Err(error) = file.persist(path) {
        tracing::error!(
            ?error,
            config.dir =? config_dir,
            message_id = "Ohquahj4",
            "config::save could not persist the temporary file"
        );
        return Err(ConfigSaveError::TempFilePersistError(error));
    }

    tracing::info!(
        config.dir =? config_dir,
        message_id = "QFVysa6j",
        "config::save successfully wrote the config",
    );

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
    pub api_host_alternate: Option<String>,
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
    pub feature_flags: FeatureFlags,
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
    pub last_exit_selector: ExitSelector,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub sni_relay: Option<String>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub wireguard_key_cache: WireGuardKeyCache,
    #[serde(skip)]
    pub use_wireguard_key_cache: (), // Removed
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub cached_account_status: Option<AccountStatus>,
    #[serde(skip)]
    pub force_tcp_tls_relay_transport: (), // Removed
}

impl Config {
    pub fn migrate(&mut self) {
        if self.last_chosen_exit_selector == (ExitSelector::Any {})
            && let Some(exit) = &self.last_chosen_exit
        {
            self.last_chosen_exit_selector = ExitSelector::Exit { id: exit.clone() };
        }
    }
}

// Redact sensitive fields by default
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigDebug {
    pub api_host_alternate: Option<String>,
    pub api_url: Option<String>,
    pub cached_exits: Option<ConfigCached<Arc<ExitList>>>,
    pub local_tunnels_ids: Vec<String>,
    pub feature_flags: FeatureFlags,
    pub in_new_account_flow: bool,
    pub pinned_locations: Vec<PinnedLocation>,
    pub last_chosen_exit: Option<String>,
    pub last_chosen_exit_selector: ExitSelector,
    pub last_exit_selector: ExitSelector,
    pub sni_relay: Option<String>,
    pub use_wireguard_key_cache: (),
    pub has_account_id: bool,
    pub has_cached_auth_token: bool,
    pub auto_connect: bool,
}

impl From<Config> for ConfigDebug {
    fn from(config: Config) -> Self {
        let Config {
            api_host_alternate,
            api_url,
            account_id,
            old_account_ids: _,
            local_tunnels_ids,
            exit: (),
            feature_flags,
            in_new_account_flow,
            cached_auth_token,
            cached_exits,
            pinned_locations,
            last_chosen_exit,
            last_chosen_exit_selector,
            last_exit_selector,
            sni_relay,
            wireguard_key_cache: _,
            use_wireguard_key_cache,
            cached_account_status: _,
            auto_connect,
            force_tcp_tls_relay_transport: (),
        } = config;
        Self {
            api_url,
            cached_exits,
            local_tunnels_ids,
            feature_flags,
            in_new_account_flow,
            pinned_locations,
            last_chosen_exit,
            last_chosen_exit_selector,
            last_exit_selector,
            api_host_alternate,
            sni_relay,
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
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct WireGuardKeyCache {
    #[serde(flatten)]
    key_pair: Option<WireGuardKeyCacheKeyPair>,
    #[serde_as(as = "Option<serde_with::TimestampSeconds>")]
    first_use: Option<SystemTime>,
    #[serde_as(as = "Option<serde_with::TimestampSeconds>")]
    registered_at: Option<SystemTime>,
    #[serde_as(as = "Vec<serde_with::base64::Base64>")]
    old_public_keys: Vec<[u8; 32]>,
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum WireGuardKeyCacheKeyPair {
    // secret key is stored in plain text in config
    Config {
        #[serde_as(as = "serde_with::base64::Base64")]
        secret_key: [u8; 32],
    },
    // secret key is stored in keychain (only public key is stored in plain text in config)
    Keychain {
        #[serde_as(as = "serde_with::base64::Base64")]
        public_key: [u8; 32],
        #[serde(skip)]
        secret_key: Option<[u8; 32]>,
    },
}

pub type KeychainSetSecretKeyFn = Box<dyn (Fn(&[u8; 32]) -> bool) + Sync + Send>;

impl core::fmt::Debug for WireGuardKeyCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { key_pair, old_public_keys, first_use, registered_at } = self;
        let (secret_key_exists, public_key) = match key_pair {
            Some(WireGuardKeyCacheKeyPair::Config { secret_key }) => {
                (true, Some(WgPubkey(PublicKey::from(&StaticSecret::from(*secret_key)).to_bytes())))
            }
            Some(WireGuardKeyCacheKeyPair::Keychain { public_key, secret_key }) => (secret_key.is_some(), Some(WgPubkey(*public_key))),
            None => (false, None),
        };
        let old_public_keys: Vec<WgPubkey> = old_public_keys.iter().map(|b| WgPubkey(*b)).collect();
        f.debug_struct("WireGuardKeyCache")
            .field("secret_key", &secret_key_exists.then_some("redacted"))
            .field("public_key", &public_key)
            .field("first_use", first_use)
            .field("registered_at", registered_at)
            .field("old_public_keys", &old_public_keys)
            .finish()
    }
}

impl WireGuardKeyCache {
    /// Sets secret key to provided value if it matches the known public key.
    pub fn try_set_secret_key_from_keychain(&mut self, keychain_secret_key: Option<&[u8]>) {
        let Some(keychain_secret_key) = keychain_secret_key else {
            tracing::info!(message_id = "0wth7DUt", "no secret key from keychain provided");
            return;
        };
        let Ok(keychain_secret_key): Result<[u8; 32], _> = keychain_secret_key.try_into() else {
            tracing::error!(
                message_id = "qEGlqS8N",
                "provided secret key from keychain has wrong length: {}",
                keychain_secret_key.len()
            );
            return;
        };
        match self.key_pair {
            Some(WireGuardKeyCacheKeyPair::Config { secret_key: _ } | WireGuardKeyCacheKeyPair::Keychain { public_key: _, secret_key: Some(_) }) => {
                tracing::error!(message_id = "9BXU4iWo", "secret already set ignoring secret key from keychain");
            }
            Some(WireGuardKeyCacheKeyPair::Keychain { public_key, secret_key: None }) => {
                let keychain_secret_key = StaticSecret::from(keychain_secret_key);
                let keychain_public_key = PublicKey::from(&keychain_secret_key);
                if keychain_public_key.as_bytes() == &public_key {
                    tracing::info!(message_id = "5ZRaCxBA", "secret key from keychain matches public key");
                    self.key_pair = Some(WireGuardKeyCacheKeyPair::Keychain { secret_key: Some(keychain_secret_key.to_bytes()), public_key });
                } else {
                    tracing::error!(
                        message_id = "SzJPkoJA",
                        "public key does not match secret key from keychain, ignoring secret key from keychain"
                    );
                }
            }
            None => tracing::error!(message_id = "S6uSP4ql", "no key pair set, ignoring secret key from keychain"),
        }
    }
    fn ensure_key_pair(&mut self, set_keychain_wg_sk: Option<&KeychainSetSecretKeyFn>) -> (StaticSecret, PublicKey) {
        match self.key_pair {
            Some(WireGuardKeyCacheKeyPair::Config { secret_key })
            | Some(WireGuardKeyCacheKeyPair::Keychain { public_key: _, secret_key: Some(secret_key) }) => {
                let secret_key = StaticSecret::from(secret_key);
                let public_key = PublicKey::from(&secret_key);
                (secret_key, public_key)
            }
            Some(WireGuardKeyCacheKeyPair::Keychain { public_key: _, secret_key: None }) => {
                tracing::error!(
                    message_id = "804Y3Qdi",
                    "only public wireguard key is known, initialization from keychain failed at load"
                );
                self.rotate_now_internal(set_keychain_wg_sk)
            }
            None => {
                tracing::info!(message_id = "RbSiOlzl", "no wireguard key pair exists yet");
                self.rotate_now_internal(set_keychain_wg_sk)
            }
        }
    }
    pub fn use_key_pair(&mut self, set_keychain_wg_sk: Option<&KeychainSetSecretKeyFn>) -> (StaticSecret, PublicKey) {
        let (secret_key, public_key) = self.ensure_key_pair(set_keychain_wg_sk);
        let now = SystemTime::now();
        self.first_use.get_or_insert(now);
        (secret_key, public_key)
    }
    pub fn rotate_now(&mut self, set_keychain_wg_sk: Option<&KeychainSetSecretKeyFn>) {
        self.rotate_now_internal(set_keychain_wg_sk);
    }
    fn rotate_now_internal(&mut self, set_keychain_wg_sk: Option<&KeychainSetSecretKeyFn>) -> (StaticSecret, PublicKey) {
        tracing::info!("rotating wireguard key pair");
        let mut old_public_keys = std::mem::take(&mut self.old_public_keys);
        let current_public_key = match self.key_pair {
            Some(WireGuardKeyCacheKeyPair::Config { secret_key }) => Some(PublicKey::from(&StaticSecret::from(secret_key)).to_bytes()),
            Some(WireGuardKeyCacheKeyPair::Keychain { public_key, secret_key: _ }) => Some(public_key),
            None => None,
        };
        if let Some(current_public_key) = current_public_key {
            old_public_keys.push(current_public_key);
        }

        let secret_key = StaticSecret::random_from_rng(OsRng);
        let public_key = PublicKey::from(&secret_key);
        let key_pair = if let Some(set_keychain_wg_sk) = set_keychain_wg_sk {
            if !set_keychain_wg_sk(&secret_key.to_bytes()) {
                tracing::error!(message_id = "WuqX5xSE", "failed to set secret key in keychain");
            }
            WireGuardKeyCacheKeyPair::Keychain { public_key: public_key.to_bytes(), secret_key: Some(secret_key.to_bytes()) }
        } else {
            WireGuardKeyCacheKeyPair::Config { secret_key: secret_key.to_bytes() }
        };

        *self = Self { key_pair: Some(key_pair), first_use: None, registered_at: None, old_public_keys };
        (secret_key, public_key)
    }
    pub fn rotate_if_required(&mut self, set_keychain_wg_sk: Option<&KeychainSetSecretKeyFn>) {
        const MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30); // 30 days
        if self.first_use.is_some_and(|t| t.elapsed().is_ok_and(|age| age > MAX_AGE)) {
            self.rotate_now(set_keychain_wg_sk);
        } else {
            tracing::info!("no wireguard key pair rotation required");
        }
    }
    pub fn need_registration(&mut self, set_keychain_wg_sk: Option<&KeychainSetSecretKeyFn>) -> Option<(PublicKey, Vec<PublicKey>)> {
        let (_, public_key) = self.ensure_key_pair(set_keychain_wg_sk);
        if self.registered_at.is_none() {
            let old_public_keys = self.old_public_keys.iter().copied().map(Into::into).collect();
            return Some((public_key, old_public_keys));
        }
        None
    }
    pub fn registered(&mut self, registered_public_key: PublicKey, removed_public_keys: &[PublicKey]) {
        let current_public_key = match self.key_pair {
            Some(WireGuardKeyCacheKeyPair::Config { secret_key }) => Some(PublicKey::from(&StaticSecret::from(secret_key))),
            Some(WireGuardKeyCacheKeyPair::Keychain { public_key, secret_key: _ }) => Some(PublicKey::from(public_key)),
            None => None,
        };
        if Some(registered_public_key) == current_public_key {
            self.registered_at = Some(SystemTime::now());
        }
        self.old_public_keys.retain(|b| !removed_public_keys.contains(&PublicKey::from(*b)));
    }
}
