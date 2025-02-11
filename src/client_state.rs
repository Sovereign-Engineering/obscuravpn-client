use boringtun::x25519::{PublicKey, StaticSecret};
use chrono::Utc;
use obscuravpn_api::cmd::CacheWgKey;
use obscuravpn_api::types::{AccountId, AccountInfo, AuthToken, OneExit};
use obscuravpn_api::{
    cmd::{ApiErrorKind, Cmd, CreateTunnel, DeleteTunnel, ListRelays, ListTunnels},
    types::{ObfuscatedTunnelConfig, OneRelay, TunnelConfig, WgPubkey},
    Client, ClientError,
};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::{
    mem,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::spawn;
use uuid::Uuid;

use super::{
    errors::{ApiError, TunnelConnectError},
    network_config::NetworkConfig,
};
use crate::config::ConfigSaveError;
use crate::config::PinnedLocation;
use crate::quicwg::QuicWgConnHandshaking;
use crate::relay_selection::race_relay_handshakes;
use crate::{
    config::{self, Config, ConfigLoadError},
    errors::RelaySelectionError,
    quicwg::QuicWgConn,
};

pub struct ClientState {
    user_agent: String,
    inner: Mutex<ClientStateInner>,
}

struct ClientStateInner {
    config_dir: PathBuf,
    config: Config,
    cached_api_client: Option<Arc<Client>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AccountStatus {
    pub account_info: AccountInfo, // API
    pub last_updated_sec: u64,
}

impl Eq for AccountStatus {}

impl PartialEq for AccountStatus {
    fn eq(&self, other: &Self) -> bool {
        self.last_updated_sec == other.last_updated_sec
    }
}

impl ClientStateInner {
    fn base_url(&self) -> String {
        self.config.api_url.clone().unwrap_or(crate::DEFAULT_API_URL.to_string())
    }
}

impl ClientState {
    pub fn new(config_dir: PathBuf, old_config_dir: PathBuf, user_agent: String) -> Result<Self, ConfigLoadError> {
        let config = config::load(&config_dir, &old_config_dir)?;
        let inner = ClientStateInner { config_dir, config, cached_api_client: None };
        Ok(Self { user_agent, inner: Mutex::new(inner) })
    }

    fn lock(&self) -> MutexGuard<ClientStateInner> {
        self.inner.lock().unwrap()
    }

    fn change_config<T>(inner: &mut ClientStateInner, f: impl FnOnce(&mut Config) -> T) -> Result<T, ConfigSaveError> {
        let mut new_config = inner.config.clone();
        let ret = f(&mut new_config);
        if inner.config != new_config {
            config::save(&inner.config_dir, &new_config)?;
        }
        inner.config = new_config;
        Ok(ret)
    }

    /// Log in or out.
    ///
    /// If `account_id` is set log in `auth_token` may be specified with an initial auth token.
    ///
    /// If `account_id` is `None` log out, `auth_token` should be `None`.
    pub fn set_account_id(&self, account_id: Option<AccountId>, auth_token: Option<AuthToken>) -> Result<(), ConfigSaveError> {
        debug_assert!(
            account_id.is_some() || auth_token.is_none(),
            "It doesn't make sense to set `auth_token` with no `account_id`."
        );

        let mut inner = self.lock();
        inner.cached_api_client = None;
        Self::change_config(&mut inner, move |config| {
            if account_id != config.account_id {
                // Log-out / Change User

                let mut old_account_ids = mem::take(&mut config.old_account_ids);
                if let Some(old_account_id) = &config.account_id {
                    if !old_account_ids.contains(old_account_id) {
                        old_account_ids.push(old_account_id.clone());
                    }
                }

                *config = Config {
                    api_url: config.api_url.take(),
                    account_id,
                    cached_auth_token: auth_token.map(Into::into),
                    old_account_ids,
                    in_new_account_flow: config.in_new_account_flow,
                    // see https://linear.app/soveng/issue/OBS-1171
                    local_tunnels_ids: config.local_tunnels_ids.clone(),
                    ..Default::default()
                }
            } else {
                tracing::warn!(message_id = "shia4Eph", "Setting auth token for logged in account. This isn't expected.");
                config.cached_auth_token = auth_token.map(Into::into);
            }
        })?;
        Ok(())
    }

    pub fn get_config(&self) -> Config {
        self.lock().config.clone()
    }

    pub fn maybe_migrate_pinned_exits(&self, exits: &obscuravpn_api::cmd::ExitList) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        if inner.config.pinned_locations.is_some() {
            return Ok(());
        }

        tracing::info!(
            message_id = "aezee9No",
            pinned_exits =? &inner.config.pinned_exits,
            "Migrating pinned exits."
        );

        let exits_by_id = exits
            .exits
            .iter()
            .map(|exit| (&exit.id, exit))
            .collect::<std::collections::HashMap<_, _>>();

        let mut duplicates = std::collections::HashSet::new();

        let mut pinned_locations = Vec::new();
        for pin in &inner.config.pinned_exits {
            let Some(exit) = exits_by_id.get(pin) else {
                tracing::warn!(
                    message_id = "eich1Uo5",
                    exit.id =? pin,
                    "Pinned exit not in exit list, ignoring",
                );
                continue;
            };

            // While we should assume the possibility of duplicates in the pin list in general we will remove duplicates during the migration.
            if !duplicates.insert((&exit.country_code, &exit.city_code)) {
                tracing::info!(
                    message_id = "ohPh5obi",
                    exit.id = pin,
                    exit.country_code = exit.country_code,
                    exit.city_code = exit.city_code,
                    "Duplicate exit for location ignored.",
                );
                continue;
            }

            pinned_locations.push(PinnedLocation {
                country_code: exit.country_code.clone(),
                city_code: exit.city_code.clone(),
                pinned_at: SystemTime::UNIX_EPOCH,
            });
        }

        tracing::info!(
            message_id = "aca0CeiY",
            pinned_locations =? pinned_locations,
            "Pinned exits migration complete.",
        );

        Self::change_config(&mut inner, move |config| config.pinned_locations = Some(pinned_locations))?;

        Ok(())
    }

    pub fn set_pinned_locations(&self, pinned_locations: Vec<PinnedLocation>) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config| {
            config.pinned_locations = Some(pinned_locations);
        })?;
        Ok(())
    }

    pub fn set_in_new_account_flow(&self, value: bool) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config| config.in_new_account_flow = value)?;
        Ok(())
    }

    pub fn set_api_url(&self, url: Option<String>) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config| config.api_url = url)?;
        inner.cached_api_client = None;
        Ok(())
    }

    pub(crate) async fn connect(&self, exit: Option<String>) -> Result<(QuicWgConn, NetworkConfig, OneExit, OneRelay), TunnelConnectError> {
        let chose_exit = exit.is_some();
        let (token, tunnel_config, wg_sk, exit, relay, handshaking) = self.new_tunnel(exit).await?;
        let network_config = NetworkConfig::new(&tunnel_config)?;
        let client_ip_v4 = network_config.ipv4;
        tracing::info!(
            tunnel.id =% token,
            exit.pubkey =? tunnel_config.exit_pubkey,
            "finishing tunnel connection");
        let remote_pk = PublicKey::from(tunnel_config.exit_pubkey.0);
        let ping_keepalive_ip = tunnel_config.gateway_ip_v4;
        let conn = QuicWgConn::connect(handshaking, wg_sk.clone(), remote_pk, client_ip_v4, ping_keepalive_ip, token).await?;
        tracing::info!("tunnel connected");
        if chose_exit {
            Self::change_config(&mut self.lock(), |config| config.last_chosen_exit = Some(exit.id.clone()))?;
        }
        Ok((conn, network_config, exit, relay))
    }

    async fn new_tunnel(
        &self,
        exit: Option<String>,
    ) -> anyhow::Result<(Uuid, ObfuscatedTunnelConfig, StaticSecret, OneExit, OneRelay, QuicWgConnHandshaking), TunnelConnectError> {
        let (closest_relay, handshaking) = self.select_relay().await?;
        let exit = exit.or_else(|| closest_relay.preferred_exits.choose(&mut thread_rng()).map(|e| e.id.clone()));

        let (tunnel_info, sk, tunnel_id) = loop {
            if let Err(err) = self.remove_local_tunnels().await {
                tracing::warn!("error removing unused local tunnels: {}", err);
            }

            let (sk, pk) = Self::change_config(&mut self.lock(), |config| config.wireguard_key_cache.use_key_pair())?;
            let wg_pubkey = WgPubkey(pk.to_bytes());
            let tunnel_id = Uuid::new_v4();
            tracing::info!(
                    %tunnel_id,
                    client.pubkey =? wg_pubkey,
                    exit.id = exit.as_deref(),
                    relay.id =? &closest_relay.id,
                    relay.ip_v4 =% closest_relay.ip_v4,
                    relay.ip_v6 =% closest_relay.ip_v6,
                    "creating tunnel");
            Self::change_config(&mut self.lock(), |config| config.local_tunnels_ids.push(tunnel_id.to_string()))?;

            let cmd = CreateTunnel::Obfuscated { id: Some(tunnel_id), wg_pubkey, relay: Some(closest_relay.id.clone()), exit: exit.clone() };
            let error = match self.api_request(cmd.clone()).await {
                Ok(t) => break (t, sk, tunnel_id),
                Err(error) => match error.api_error_kind() {
                    Some(ApiErrorKind::TunnelLimitExceeded {}) => error,
                    Some(ApiErrorKind::WgKeyRotationRequired {}) => {
                        tracing::warn!(?error, "server indicated that key rotation is required immediately");
                        Self::change_config(&mut self.lock(), |config| config.wireguard_key_cache.rotate_now())?;
                        continue;
                    }
                    _ => return Err(error.into()),
                },
            };
            tracing::warn!(?error, "no tunnel slots left, trying to delete an unused one");
            let last_used_threshold = Utc::now().timestamp() - 300;
            let mut tunnels: Vec<(String, i64)> = self
                .api_request(ListTunnels {})
                .await?
                .into_iter()
                .filter_map(|t| match &t.config {
                    TunnelConfig::Obfuscated(_) => {
                        use obscuravpn_api::types::TunnelStatus::*;
                        let (Created { when } | Connected { when } | Disconnected { when }) = t.status;
                        (when < last_used_threshold).then_some((t.id, when))
                    }
                    _ => None,
                })
                .collect();
            tunnels.sort_by_key(|t| t.1);
            let Some(id) = tunnels.into_iter().next().map(|t| t.0) else {
                tracing::warn!("no unused obfuscated tunnel found");
                return Err(error.into());
            };
            tracing::warn!("deleting unused tunnel {}", &id);
            self.api_request(DeleteTunnel { id }).await?;
        };

        if tunnel_info.relay.id != closest_relay.id {
            return Err(TunnelConnectError::UnexpectedRelay);
        }
        let TunnelConfig::Obfuscated(config) = tunnel_info.config else {
            return Err(TunnelConnectError::UnexpectedTunnelKind);
        };
        Ok((tunnel_id, config, sk, tunnel_info.exit, tunnel_info.relay, handshaking))
    }

    pub async fn remove_local_tunnels(&self) -> Result<(), ApiError> {
        loop {
            let Some(local_tunnel_id) = self.lock().config.local_tunnels_ids.first().cloned() else {
                return Ok(());
            };
            tracing::info!("removing previously used tunnel {}", &local_tunnel_id);
            self.api_request(DeleteTunnel { id: local_tunnel_id.clone() }).await?;
            Self::change_config(&mut self.lock(), |config| config.local_tunnels_ids.retain(|x| x != &local_tunnel_id))?
        }
    }

    pub async fn select_relay(&self) -> Result<(OneRelay, QuicWgConnHandshaking), TunnelConnectError> {
        let relays = self.api_request(ListRelays {}).await?;
        tracing::info!("relay candidates: {:?}", relays);

        let racing_handshakes = race_relay_handshakes(relays)?;
        let mut relays_connected_successfully = BTreeSet::new();
        let mut best_candidate = None;

        while let Ok((relay, port, rtt, handshaking)) = racing_handshakes.recv_async().await {
            relays_connected_successfully.insert(relay.id.clone());
            let rejected = if best_candidate.as_ref().is_some_and(|(_, _, best_rtt, _)| *best_rtt < rtt) {
                Some(handshaking)
            } else {
                best_candidate
                    .replace((relay, port, rtt, handshaking))
                    .map(|(_, _, _, replaced)| replaced)
            };
            if let Some(rejected) = rejected {
                spawn(rejected.abandon());
            }
            if relays_connected_successfully.len() >= 5 {
                break;
            }
        }

        let Some((relay, port, rtt, handshaking)) = best_candidate else {
            return Err(RelaySelectionError::NoSuccess.into());
        };
        tracing::info!(relay.id, port, rtt = rtt.as_millis(), "selected relay");
        Ok((relay, handshaking))
    }

    pub async fn api_request<C: Cmd>(&self, cmd: C) -> Result<C::Output, ApiError> {
        let api_client = {
            // MUST NOT BLOCK UNTIL `MutexGuard` IS DROPPED
            let mut inner: MutexGuard<'_, ClientStateInner> = self.lock();
            let Some(account_id) = inner.config.account_id.clone() else {
                return Err(ApiError::NoAccountId);
            };
            if let Some(api_client) = inner.cached_api_client.clone() {
                api_client
            } else {
                let base_url = inner.base_url();
                let api_client = Arc::new(Client::new(base_url, account_id, &self.user_agent).map_err(ClientError::from)?);
                if let Some(auth_token) = inner.config.cached_auth_token.clone() {
                    api_client.set_auth_token(Some(auth_token.into()));
                }
                inner.cached_api_client.insert(api_client).clone()
            }
            // IMPLICITLY DROPPING `MutexGuard`
        };

        let result = api_client.run(cmd).await;

        // MUST NOT BLOCK UNTIL `MutexGuard` IS DROPPED
        let mut inner: MutexGuard<'_, ClientStateInner> = self.lock();
        let auth_token = inner.cached_api_client.clone().and_then(|c| c.get_auth_token());
        Self::change_config(&mut inner, |config| config.cached_auth_token = auth_token.map(Into::into))?;
        drop(inner);
        // DROPPED `MutexGuard`

        Ok(result?)
    }

    pub fn base_url(&self) -> String {
        self.lock().base_url()
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    pub fn update_account_info(&self, account_info: &AccountInfo) -> Result<(), ConfigSaveError> {
        let response_time = SystemTime::now();
        let last_updated_sec = response_time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_secs();
        let mut inner = self.lock();
        let account = Some(AccountStatus { account_info: account_info.clone(), last_updated_sec });
        Self::change_config(&mut inner, move |config| {
            config.cached_account_status = account;
        })
    }

    // Only intended to be called after use (on disconnect). Rotation schedules are fairly arbitrary, so using the key one more time is fine. The benefit is that we don't trigger rotation if the user stops using the client, but the client is still auto-starting. This does not imply the effect of `Self::register_cached_wireguard_key_if_new`. It's the callers responsibility to ensure that registration is triggered asap.
    pub fn rotate_wireguard_key_if_required(&self) -> Result<(), ConfigSaveError> {
        Self::change_config(&mut self.lock(), |config| {
            config.wireguard_key_cache.rotate_if_required();
        })
    }

    // Registers the current wireguard key via the API server if it has not been registered yet. Because this function is a NOOP after first successful use (until key rotation), it may be called frequently. Most importantly it should be called after disconnecting (due to possible key rotation) and after observing that the user paid.
    pub async fn register_cached_wireguard_key_if_new(&self) -> Result<(), ApiError> {
        let Some((current_public_key, old_public_keys)) = self.get_config().wireguard_key_cache.need_registration() else {
            tracing::info!("public wireguard key already registered");
            return Ok(());
        };
        let cmd = CacheWgKey {
            public_key: WgPubkey(current_public_key.to_bytes()),
            previous_public_keys: old_public_keys.iter().map(|p| WgPubkey(p.to_bytes())).collect(),
        };
        match self.api_request(cmd).await {
            Ok(()) => {
                Self::change_config(&mut self.lock(), |config| config.wireguard_key_cache.registered(&old_public_keys))?;
                tracing::info!("successfully registered public wireguard key");
                Ok(())
            }
            Err(error) => {
                if matches!(error.api_error_kind(), Some(ApiErrorKind::WgKeyRotationRequired {})) {
                    tracing::warn!(?error, "server indicated that key rotation is required immediately");
                    Self::change_config(&mut self.lock(), |config| config.wireguard_key_cache.rotate_now())?;
                }
                Err(error)
            }
        }
    }

    pub fn rotate_wg_key(&self) -> Result<(), ConfigSaveError> {
        Self::change_config(&mut self.lock(), |config| {
            config.wireguard_key_cache.rotate_now();
        })
    }
}
