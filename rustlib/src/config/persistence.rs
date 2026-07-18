//! Atomically load, migrate and save configurations

use std::fs;
use std::fs::create_dir_all;
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use crate::client_state::AccountStatus;
use crate::config::cached::ConfigCached;
use crate::config::dns_cache::DnsCache;
use crate::config::feature_flags::FeatureFlags;
use crate::exit_selection::ExitSelector;
use crate::manager::TunnelArgs;
use crate::network_config::{DnsConfig, DnsContentBlock};
use crate::wg_key_store::{PlaintextWgSecretKey, SealedWgSecretKey, WgKeyStore};
use boringtun::x25519::StaticSecret;
use chrono::Utc;
use obscuravpn_api::cmd::ExitList;
use obscuravpn_api::types::{AccountId, WgPubkey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use strum::EnumIs;
use tempfile::{NamedTempFile, PersistError};
use thiserror::Error;

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
pub fn load(config_dir: &Path, key_store: &WgKeyStore) -> Result<Config, ConfigLoadError> {
    let path = Path::new(config_dir).join(CONFIG_FILE);

    let err = match try_load(&path) {
        Ok(config) => {
            let mut config = config.unwrap_or_default();
            config.wireguard_key_cache.try_set_secret_key(key_store);
            tracing::info!(
                config.dir =? config_dir,
                message_id = "q9XZcBvj",
                "config::load successfully loaded the config",
            );
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
                    message_id = "Szp2BpwR",
                    "Reading config file returned save error.");
                return Err(error);
            }
        },
    };

    // This may collide if failing in a tight loop, that is fine. Possibly even a feature.
    let backup_path = Path::new(config_dir).join(format!("config-backup-{}.json", Utc::now().format("%Y-%m-%dT%H-%M-%S%.3f")));

    // TODO: Do we want to try to clean up old backup configs?

    match fs::rename(&path, &backup_path) {
        Ok(()) => {}
        Err(error) => {
            tracing::error!(
                message_id = "3ABMLYMb",
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
    pub dns_content_block: DnsContentBlock,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub dns_cache: DnsCache,
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
    pub tunnel_active: bool,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub tunnel_args: TunnelArgs,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub sni_relay: Option<String>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub wireguard_key_cache: WireGuardKeyCache,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub dns: DnsConfig,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub local_network_access: LocalNetworkAccess,
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

#[derive(Clone, Copy, Debug, Default, EnumIs, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalNetworkAccess {
    #[default]
    Enabled,
    Disabled,
}

// Redact sensitive fields by default
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigDebug {
    pub api_host_alternate: Option<String>,
    pub api_url: Option<String>,
    pub cached_exits: Option<ConfigCached<Arc<ExitList>>>,
    pub dns_cache: DnsCache,
    pub dns_content_block: DnsContentBlock,
    pub local_tunnels_ids: Vec<String>,
    pub feature_flags: FeatureFlags,
    pub in_new_account_flow: bool,
    pub pinned_locations: Vec<PinnedLocation>,
    pub last_chosen_exit: Option<String>,
    pub last_chosen_exit_selector: ExitSelector,
    pub last_exit_selector: ExitSelector,
    pub sni_relay: Option<String>,
    pub tunnel_active: bool,
    pub tunnel_args: TunnelArgs,
    pub dns: DnsConfig,
    pub local_network_access: LocalNetworkAccess,
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
            dns_content_block,
            dns_cache,
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
            dns,
            local_network_access,
            use_wireguard_key_cache: (),
            cached_account_status: _,
            auto_connect,
            force_tcp_tls_relay_transport: (),
            tunnel_active,
            tunnel_args,
        } = config;
        Self {
            api_url,
            cached_exits,
            dns_content_block,
            dns_cache,
            local_tunnels_ids,
            feature_flags,
            in_new_account_flow,
            pinned_locations,
            last_chosen_exit,
            last_chosen_exit_selector,
            last_exit_selector,
            api_host_alternate,
            sni_relay,
            dns,
            local_network_access,
            has_account_id: account_id.is_some(),
            has_cached_auth_token: cached_auth_token.is_some(),
            auto_connect,
            tunnel_active,
            tunnel_args,
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
    old_public_keys: Vec<WgPubkey>,
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, strum::IntoStaticStr)]
#[serde(tag = "type", rename_all = "snake_case", from = "UntaggedWireGuardKeyCacheKeyPair")]
#[strum(serialize_all = "snake_case")]
pub enum WireGuardKeyCacheKeyPair {
    // secret key is stored in plain text in config
    Plaintext {
        secret_key: PlaintextWgSecretKey,
    },
    // secret key is stored in config sealed with the machine's TPM
    Sealed {
        sealed_secret_key: SealedWgSecretKey,
        public_key: WgPubkey,
        #[serde(skip)]
        secret_key: Option<PlaintextWgSecretKey>,
    },
    // secret key is not persisted in config (held in keychain or in memory only)
    Detached {
        public_key: WgPubkey,
        #[serde(skip)]
        secret_key: Option<PlaintextWgSecretKey>,
    },
}

// TODO: Remove after a transition period. Only needed to parse configs written before the "type" tag.
#[serde_with::serde_as]
#[derive(Deserialize)]
#[serde(untagged)]
pub enum UntaggedWireGuardKeyCacheKeyPair {
    Plaintext {
        secret_key: PlaintextWgSecretKey,
    },
    Sealed {
        sealed_secret_key: SealedWgSecretKey,
        public_key: WgPubkey,
    },
    Detached {
        public_key: WgPubkey,
    },
}

impl From<UntaggedWireGuardKeyCacheKeyPair> for WireGuardKeyCacheKeyPair {
    fn from(key_pair: UntaggedWireGuardKeyCacheKeyPair) -> Self {
        match key_pair {
            UntaggedWireGuardKeyCacheKeyPair::Plaintext { secret_key } => Self::Plaintext { secret_key },
            UntaggedWireGuardKeyCacheKeyPair::Sealed { sealed_secret_key, public_key } => {
                Self::Sealed { sealed_secret_key, public_key, secret_key: None }
            }
            UntaggedWireGuardKeyCacheKeyPair::Detached { public_key } => Self::Detached { public_key, secret_key: None },
        }
    }
}

impl core::fmt::Debug for WireGuardKeyCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { key_pair, old_public_keys, first_use, registered_at } = self;
        let (secret_key_exists, public_key) = match key_pair {
            Some(WireGuardKeyCacheKeyPair::Plaintext { secret_key }) => (true, Some(secret_key.public_key())),
            Some(WireGuardKeyCacheKeyPair::Sealed { sealed_secret_key: _, public_key, secret_key }) => (secret_key.is_some(), Some(*public_key)),
            Some(WireGuardKeyCacheKeyPair::Detached { public_key, secret_key }) => (secret_key.is_some(), Some(*public_key)),
            None => (false, None),
        };
        f.debug_struct("WireGuardKeyCache")
            .field("secret_key", &secret_key_exists.then_some("redacted"))
            .field("public_key", &public_key)
            .field("first_use", first_use)
            .field("registered_at", registered_at)
            .field("old_public_keys", old_public_keys)
            .finish()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RotationReason {
    ApiRequested,
    ApiUrlChanged,
    SecretUnavailable,
    Manual,
    NoKeyPair,
    ScheduleBased,
}

impl WireGuardKeyCache {
    fn try_set_secret_key(&mut self, key_store: &WgKeyStore) {
        match (key_store, &mut self.key_pair) {
            (_, None) => {}
            (WgKeyStore::Plaintext, Some(WireGuardKeyCacheKeyPair::Plaintext { secret_key: _ })) => {}
            (WgKeyStore::None, Some(WireGuardKeyCacheKeyPair::Detached { public_key: _, secret_key: _ })) => {}
            (WgKeyStore::Sealed(sealing_key), Some(WireGuardKeyCacheKeyPair::Sealed { sealed_secret_key, public_key, secret_key })) => {
                match sealing_key.unseal(sealed_secret_key) {
                    Ok(unsealed_secret_key) => {
                        if unsealed_secret_key.public_key() == *public_key {
                            tracing::info!(message_id = "mF8xVd3J", "unsealed wireguard secret key matches stored public key");
                            *secret_key = Some(unsealed_secret_key);
                        } else {
                            tracing::error!(message_id = "yN3rWv7q", "unsealed wireguard secret key does not match stored public key");
                        }
                    }
                    Err(()) => {
                        tracing::error!(message_id = "Dk3wZr7M", "failed to unseal wireguard secret key");
                    }
                }
            }
            (
                WgKeyStore::Keychain { secret_key: keychain_secret_key, set_secret_key: _ },
                Some(WireGuardKeyCacheKeyPair::Detached { public_key, secret_key }),
            ) => {
                let Some(keychain_secret_key) = keychain_secret_key else {
                    tracing::info!(message_id = "0wth7DUt", "no secret key from keychain provided");
                    return;
                };
                let Ok(keychain_secret_key): Result<[u8; 32], _> = keychain_secret_key.as_slice().try_into() else {
                    tracing::error!(
                        length = keychain_secret_key.len(),
                        message_id = "qEGlqS8N",
                        "provided secret key from keychain has wrong length"
                    );
                    return;
                };
                let keychain_secret_key = PlaintextWgSecretKey::new(keychain_secret_key);
                if keychain_secret_key.public_key() == *public_key {
                    tracing::info!(message_id = "5ZRaCxBA", "secret key from keychain matches public key");
                    *secret_key = Some(keychain_secret_key);
                } else {
                    tracing::error!(
                        message_id = "SzJPkoJA",
                        "public key does not match secret key from keychain, ignoring secret key from keychain"
                    );
                }
            }
            (key_store, Some(key_pair)) => {
                let key_store_type: &'static str = key_store.into();
                let key_pair_type: &'static str = (&*key_pair).into();
                tracing::warn!(
                    key_pair_type,
                    key_store_type,
                    message_id = "hV2mQx9c",
                    "persisted wireguard key pair does not match key store"
                );
            }
        }
    }
    fn ensure_key_pair(&mut self, key_store: &WgKeyStore) -> (StaticSecret, WgPubkey) {
        match &self.key_pair {
            Some(
                WireGuardKeyCacheKeyPair::Plaintext { secret_key }
                | WireGuardKeyCacheKeyPair::Sealed { sealed_secret_key: _, public_key: _, secret_key: Some(secret_key) }
                | WireGuardKeyCacheKeyPair::Detached { public_key: _, secret_key: Some(secret_key) },
            ) => (secret_key.static_secret(), secret_key.public_key()),
            Some(
                WireGuardKeyCacheKeyPair::Sealed { sealed_secret_key: _, public_key: _, secret_key: None }
                | WireGuardKeyCacheKeyPair::Detached { public_key: _, secret_key: None },
            ) => self.rotate_now_internal(RotationReason::SecretUnavailable, key_store),
            None => self.rotate_now_internal(RotationReason::NoKeyPair, key_store),
        }
    }
    pub fn use_key_pair(&mut self, key_store: &WgKeyStore) -> (StaticSecret, WgPubkey) {
        let (secret_key, public_key) = self.ensure_key_pair(key_store);
        let now = SystemTime::now();
        self.first_use.get_or_insert(now);
        (secret_key, public_key)
    }
    pub fn rotate_now(&mut self, reason: RotationReason, key_store: &WgKeyStore) {
        self.rotate_now_internal(reason, key_store);
    }
    fn rotate_now_internal(&mut self, reason: RotationReason, key_store: &WgKeyStore) -> (StaticSecret, WgPubkey) {
        tracing::info!(message_id = "65KkXAbB", ?reason, "rotating wireguard key pair");
        let mut old_public_keys = std::mem::take(&mut self.old_public_keys);
        let current_public_key = match &self.key_pair {
            Some(WireGuardKeyCacheKeyPair::Plaintext { secret_key }) => Some(secret_key.public_key()),
            Some(WireGuardKeyCacheKeyPair::Sealed { sealed_secret_key: _, public_key, secret_key: _ }) => Some(*public_key),
            Some(WireGuardKeyCacheKeyPair::Detached { public_key, secret_key: _ }) => Some(*public_key),
            None => None,
        };
        if let Some(current_public_key) = current_public_key {
            old_public_keys.push(current_public_key);
        }

        let secret_key = PlaintextWgSecretKey::new(StaticSecret::random_from_rng(OsRng).to_bytes());
        let public_key = secret_key.public_key();
        let static_secret = secret_key.static_secret();
        let key_pair = match key_store {
            WgKeyStore::Plaintext => WireGuardKeyCacheKeyPair::Plaintext { secret_key },
            WgKeyStore::Keychain { secret_key: _, set_secret_key } => {
                if !set_secret_key(&static_secret.to_bytes()) {
                    tracing::error!(message_id = "WuqX5xSE", "failed to set secret key in keychain");
                }
                WireGuardKeyCacheKeyPair::Detached { public_key, secret_key: Some(secret_key) }
            }
            WgKeyStore::None => {
                tracing::warn!(message_id = "qF7dLw3X", "no wireguard key store, keeping secret key in memory only");
                WireGuardKeyCacheKeyPair::Detached { public_key, secret_key: Some(secret_key) }
            }
            WgKeyStore::Sealed(sealing_key) => match sealing_key.seal(&secret_key) {
                Ok(sealed_secret_key) => WireGuardKeyCacheKeyPair::Sealed { sealed_secret_key, public_key, secret_key: Some(secret_key) },
                Err(()) => {
                    tracing::error!(message_id = "kR6wHb3N", "failed to seal wireguard secret key, keeping it in memory only");
                    WireGuardKeyCacheKeyPair::Detached { public_key, secret_key: Some(secret_key) }
                }
            },
        };

        *self = Self { key_pair: Some(key_pair), first_use: None, registered_at: None, old_public_keys };
        (static_secret, public_key)
    }
    pub fn rotate_if_required(&mut self, key_store: &WgKeyStore) {
        const MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30); // 30 days
        if self.first_use.is_some_and(|t| t.elapsed().is_ok_and(|age| age > MAX_AGE)) {
            self.rotate_now(RotationReason::ScheduleBased, key_store);
        } else {
            tracing::info!(message_id = "i85mYSwz", "no wireguard key pair rotation required");
        }
    }
    pub fn need_registration(&mut self, key_store: &WgKeyStore) -> Option<(WgPubkey, Vec<WgPubkey>)> {
        let (_, public_key) = self.ensure_key_pair(key_store);
        if self.registered_at.is_none() {
            return Some((public_key, self.old_public_keys.clone()));
        }
        None
    }
    pub fn registered(&mut self, registered_public_key: WgPubkey, removed_public_keys: &[WgPubkey]) {
        let current_public_key = match &self.key_pair {
            Some(WireGuardKeyCacheKeyPair::Plaintext { secret_key }) => Some(secret_key.public_key()),
            Some(WireGuardKeyCacheKeyPair::Sealed { sealed_secret_key: _, public_key, secret_key: _ }) => Some(*public_key),
            Some(WireGuardKeyCacheKeyPair::Detached { public_key, secret_key: _ }) => Some(*public_key),
            None => None,
        };
        if Some(registered_public_key) == current_public_key {
            self.registered_at = Some(SystemTime::now());
        }
        self.old_public_keys.retain(|b| !removed_public_keys.contains(b));
    }
}
