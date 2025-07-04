use super::{
    errors::{ApiError, TunnelConnectError},
    network_config::NetworkConfig,
};
use crate::relay_selection::race_relay_handshakes;
use crate::{config::KeychainSetSecretKeyFn, quicwg::QuicWgConnHandshaking};
use crate::{config::PinnedLocation, exit_selection::ExitSelectionState};
use crate::{
    config::{self, Config, ConfigLoadError},
    errors::RelaySelectionError,
    quicwg::QuicWgConn,
};
use crate::{
    config::{cached::ConfigCached, ConfigSaveError},
    exit_selection::ExitSelector,
};
use boringtun::x25519::{PublicKey, StaticSecret};
use chrono::Utc;
use obscuravpn_api::cmd::{CacheWgKey, ETagCmd, ExitList, ListExits2};
use obscuravpn_api::types::{AccountId, AccountInfo, AuthToken, OneExit};
use obscuravpn_api::{
    cmd::{ApiErrorKind, Cmd, CreateTunnel, DeleteTunnel, ListRelays, ListTunnels},
    types::{ObfuscatedTunnelConfig, OneRelay, TunnelConfig, WgPubkey},
    Client, ClientError,
};
use rand::{seq::SliceRandom, thread_rng};
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

const DEFAULT_API_BACKUP: &str = "crimsonlance.net";
const DEFAULT_RELAY_SNI: &str = "example.com";

pub struct ClientState {
    exit_list_watch: tokio::sync::watch::Sender<Option<ConfigCached<Arc<ExitList>>>>,
    exit_update_lock: tokio::sync::Mutex<()>,
    inner: Mutex<ClientStateInner>,
    user_agent: String,
}

struct ClientStateInner {
    cached_api_client: Option<Arc<Client>>,
    config: Config,
    config_dir: PathBuf,
    set_keychain_wg_sk: Option<KeychainSetSecretKeyFn>,
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
    pub fn new(
        config_dir: PathBuf,
        keychain_wg_sk: Option<&[u8]>,
        user_agent: String,
        set_keychain_wg_sk: Option<KeychainSetSecretKeyFn>,
    ) -> Result<Self, ConfigLoadError> {
        let mut config = config::load(&config_dir, keychain_wg_sk)?;
        config.migrate();
        let exit_list_watch = tokio::sync::watch::channel(config.cached_exits.clone()).0;
        let inner = ClientStateInner { config_dir, config, cached_api_client: None, set_keychain_wg_sk };
        Ok(Self { exit_list_watch, exit_update_lock: Default::default(), inner: Mutex::new(inner), user_agent })
    }

    fn lock(&self) -> MutexGuard<ClientStateInner> {
        self.inner.lock().unwrap()
    }

    fn change_config<T>(inner: &mut ClientStateInner, f: impl FnOnce(&mut Config, &mut ClientStateInner) -> T) -> Result<T, ConfigSaveError> {
        let mut new_config = inner.config.clone();
        let ret = f(&mut new_config, inner);
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
        Self::change_config(&mut inner, move |config, _| {
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

    pub fn get_exit_list(&self) -> Option<ConfigCached<Arc<ExitList>>> {
        self.lock().config.cached_exits.clone()
    }

    pub fn set_pinned_locations(&self, pinned_locations: Vec<PinnedLocation>) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config, _| {
            config.pinned_locations = pinned_locations;
        })?;
        Ok(())
    }

    pub fn set_feature_flag(&self, flag: &str, active: bool) -> Result<(), ConfigSaveError> {
        Self::change_config(&mut self.lock(), |config, _| {
            config.feature_flags.set(flag, active);
        })?;
        Ok(())
    }

    pub fn set_api_host_alternate(&self, value: Option<String>) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config, _| {
            tracing::info!(
                message_id = "jee1ieWa",
                api_host_alternate_new = value,
                api_host_alternate_old = config.api_host_alternate,
                "Changing API alternate host.",
            );
            config.api_host_alternate = value;
        })
    }

    pub fn set_sni_relay(&self, value: Option<String>) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config, _| {
            tracing::info!(
                message_id = "jee1ieWa",
                sni_relay_new = value,
                sni_relay_old = config.sni_relay,
                "Changing Relay SNI.",
            );
            config.sni_relay = value;
        })
    }

    pub fn set_in_new_account_flow(&self, value: bool) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config, _| config.in_new_account_flow = value)?;
        Ok(())
    }

    pub fn set_api_url(&self, url: Option<String>) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config, _| config.api_url = url)?;
        inner.cached_api_client = None;
        Ok(())
    }

    pub fn set_auto_connect(&self, enable: bool) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config, _| config.auto_connect = enable)?;
        Ok(())
    }

    pub fn set_force_tcp_tls_relay_transport(&self, enable: bool) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config, _| config.force_tcp_tls_relay_transport = enable)?;
        Ok(())
    }

    pub async fn connect(
        &self,
        exit_selector: &ExitSelector,
        selection_state: &mut ExitSelectionState,
    ) -> Result<(QuicWgConn, NetworkConfig, OneExit, OneRelay), TunnelConnectError> {
        let (token, tunnel_config, wg_sk, exit, relay, handshaking) = self.new_tunnel(exit_selector, selection_state).await?;
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
        let exit_id = exit.id.clone();
        Self::change_config(&mut self.lock(), move |config, _| {
            if *exit_selector != (ExitSelector::Any {}) {
                config.last_chosen_exit = Some(exit_id);
                config.last_chosen_exit_selector = exit_selector.clone();
            };
            config.last_exit_selector = exit_selector.clone();
        })?;
        Ok((conn, network_config, exit, relay))
    }

    fn choose_exit(&self, selector: &ExitSelector, relay: &OneRelay, selection_state: &mut ExitSelectionState) -> Option<String> {
        let Some(exit_list) = self.get_exit_list() else {
            tracing::warn!(message_id = "Iu1ahnge", "No exit list, choosing random preferred exit.");
            return relay.preferred_exits.choose(&mut thread_rng()).map(|e| e.id.clone());
        };
        selection_state
            .select_next_exit(selector, &exit_list.value.exits, relay)
            .map(|e| e.id.clone())
    }

    async fn new_tunnel(
        &self,
        exit_selector: &ExitSelector,
        selection_state: &mut ExitSelectionState,
    ) -> anyhow::Result<(Uuid, ObfuscatedTunnelConfig, StaticSecret, OneExit, OneRelay, QuicWgConnHandshaking), TunnelConnectError> {
        // Ideally we would avoid return a failure immediately if the relay selection fails and continue the exit update in the background but we currently have no ability to execute tasks in the background for this type. The downside of a slight delay in the failure case is suboptimal but minor.

        let (select_relay, update_exits) = tokio::join!(self.select_relay(), self.maybe_update_exits(Duration::from_secs(60)),);
        match update_exits {
            Ok(()) => {}
            Err(error) => {
                tracing::warn!(message_id = "oH5aigha", ?error, "Ignoring failure to update exit list: {}", error,);
            }
        };
        let (closest_relay, handshaking) = select_relay?;

        let Some(exit) = self.choose_exit(exit_selector, &closest_relay, selection_state) else {
            tracing::error!(
                message_id = "naiThei6",
                exit_selector =? exit_selector,
                "No exits matching selector."
            );
            return Err(TunnelConnectError::NoExit);
        };

        tracing::info!(
            message_id = "eiR8ixoh",
            exit.id = exit,
            exit_selector =? exit_selector,
            "Selected exit"
        );

        let (tunnel_info, sk, tunnel_id) = loop {
            if let Err(err) = self.remove_local_tunnels().await {
                tracing::warn!("error removing unused local tunnels: {}", err);
            }

            let (sk, pk) = Self::change_config(&mut self.lock(), |config, inner| {
                config.wireguard_key_cache.use_key_pair(inner.set_keychain_wg_sk.as_ref())
            })?;
            let wg_pubkey = WgPubkey(pk.to_bytes());
            let tunnel_id = Uuid::new_v4();
            tracing::info!(
                    %tunnel_id,
                    client.pubkey =? wg_pubkey,
                    exit.id = exit,
                    relay.id =? &closest_relay.id,
                    relay.ip_v4 =% closest_relay.ip_v4,
                    "creating tunnel");
            Self::change_config(&mut self.lock(), |config, _| config.local_tunnels_ids.push(tunnel_id.to_string()))?;

            let cmd = CreateTunnel::Obfuscated {
                id: Some(tunnel_id),
                label: None,
                wg_pubkey,
                relay: Some(closest_relay.id.clone()),
                exit: Some(exit.clone()),
            };
            let error = match self.api_request(cmd.clone()).await {
                Ok(t) => break (t, sk, tunnel_id),
                Err(error) => match error.api_error_kind() {
                    Some(ApiErrorKind::TunnelLimitExceeded {}) => error,
                    Some(ApiErrorKind::WgKeyRotationRequired {}) => {
                        tracing::warn!(?error, "server indicated that key rotation is required immediately");
                        Self::change_config(&mut self.lock(), |config, inner| {
                            config.wireguard_key_cache.rotate_now(inner.set_keychain_wg_sk.as_ref())
                        })?;
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
            Self::change_config(&mut self.lock(), |config, _| config.local_tunnels_ids.retain(|x| x != &local_tunnel_id))?
        }
    }

    pub async fn select_relay(&self) -> Result<(OneRelay, QuicWgConnHandshaking), TunnelConnectError> {
        let relays = self.api_request(ListRelays {}).await?;
        let sni = self.lock().config.sni_relay.clone().unwrap_or_else(|| DEFAULT_RELAY_SNI.into());

        tracing::info!(
            message_id = "eech6Ier",
            relays =? relays,
            sni = sni,
            "Racing relays",
        );
        let use_tcp_tls = self.get_config().force_tcp_tls_relay_transport;
        let pad_to_mtu = self.get_config().feature_flags.quic_frame_padding.unwrap_or(false);
        let racing_handshakes = race_relay_handshakes(relays, sni, use_tcp_tls, pad_to_mtu)?;
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

    pub fn make_api_client(&self, account_id: AccountId) -> Result<Client, ApiError> {
        let mut inner = self.lock();
        self.make_api_client_inner(&mut inner, account_id)
    }

    fn make_api_client_inner(&self, inner: &mut ClientStateInner, account_id: AccountId) -> Result<Client, ApiError> {
        let base_url = inner.base_url();
        Client::new(
            base_url,
            vec![inner.config.api_host_alternate.clone().unwrap_or_else(|| DEFAULT_API_BACKUP.into())],
            account_id,
            &self.user_agent,
        )
        .map_err(ClientError::from)
        .map_err(ApiError::from)
    }

    fn api_client(&self) -> Result<Arc<Client>, ApiError> {
        let mut inner = self.lock();

        let Some(account_id) = inner.config.account_id.clone() else {
            return Err(ApiError::NoAccountId);
        };

        if let Some(api_client) = inner.cached_api_client.clone() {
            Ok(api_client)
        } else {
            let api_client = Arc::new(self.make_api_client_inner(&mut inner, account_id)?);
            if let Some(auth_token) = inner.config.cached_auth_token.clone() {
                api_client.set_auth_token(Some(auth_token.into()));
            }
            Ok(inner.cached_api_client.insert(api_client).clone())
        }
    }

    fn cache_auth_token(&self) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();

        let auth_token = inner.cached_api_client.as_ref().and_then(|c| c.get_auth_token());
        Self::change_config(&mut inner, |config, _| {
            config.cached_auth_token = auth_token.map(Into::into);
        })?;

        Ok(())
    }

    pub async fn api_request<C: Cmd>(&self, cmd: C) -> Result<C::Output, ApiError> {
        let api_client = self.api_client()?;
        let result = api_client.run(cmd).await;
        self.cache_auth_token()?;
        Ok(result?)
    }

    pub async fn cached_api_request<C: ETagCmd>(&self, cmd: C, etag: Option<&[u8]>) -> Result<obscuravpn_api::Response<C::Output>, ApiError> {
        let api_client = self.api_client()?;
        let result = api_client.run_with_etag(cmd, etag).await?;
        self.cache_auth_token()?;
        Ok(result)
    }

    pub fn base_url(&self) -> String {
        self.lock().base_url()
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    pub async fn maybe_update_exits(&self, freshness: Duration) -> Result<(), ApiError> {
        let _update_lock = self.exit_update_lock.lock().await;

        let prev = self.lock().config.cached_exits.clone();
        let prev = prev.as_ref();
        if prev.is_some_and(|c| c.staleness() < freshness) {
            tracing::info!(message_id = "fao5ciJu", "Exit list is already up to date.");
            return Ok(());
        }

        let res = self.cached_api_request(ListExits2 {}, prev.as_ref().and_then(|p| p.etag())).await?;

        let etag = res.etag().map(|e| e.to_vec());

        let Some(body) = res.into_body() else { return Ok(()) };

        let version = match etag {
            Some(b) => config::cached::Version::ETag(b),
            None => {
                tracing::warn!(message_id = "meequa8P", "Exit list had not ETag.");
                config::cached::Version::artificial()
            }
        };
        let cached_exits = ConfigCached::new(Arc::new(body), version);

        let mut inner = self.lock();

        Self::change_config(&mut inner, |config, _| {
            config.cached_exits = Some(cached_exits.clone());
        })?;

        match self.exit_list_watch.send(Some(cached_exits)) {
            Ok(()) => {}
            Err(error) => {
                tracing::error!(?error, message_id = "Ziesha5y", "Ignoring failed exit_list_watch.send: {}", error,);
            }
        }

        Ok(())
    }

    pub fn update_account_info(&self, account_info: &AccountInfo) -> Result<(), ConfigSaveError> {
        let response_time = SystemTime::now();
        let last_updated_sec = response_time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_secs();
        let mut inner = self.lock();
        let account = Some(AccountStatus { account_info: account_info.clone(), last_updated_sec });
        Self::change_config(&mut inner, move |config, _| {
            config.cached_account_status = account;
        })
    }

    // Only intended to be called after use (on disconnect). Rotation schedules are fairly arbitrary, so using the key one more time is fine. The benefit is that we don't trigger rotation if the user stops using the client, but the client is still auto-starting. This does not imply the effect of `Self::register_cached_wireguard_key_if_new`. It's the callers responsibility to ensure that registration is triggered asap.
    pub fn rotate_wireguard_key_if_required(&self) -> Result<(), ConfigSaveError> {
        Self::change_config(&mut self.lock(), |config, inner| {
            config.wireguard_key_cache.rotate_if_required(inner.set_keychain_wg_sk.as_ref());
        })
    }

    // Registers the current wireguard key via the API server if it has not been registered yet. Because this function is a NOOP after first successful use (until key rotation), it may be called frequently. Most importantly it should be called after disconnecting (due to possible key rotation) and after observing that the user paid.
    pub async fn register_cached_wireguard_key_if_new(&self) -> Result<(), ApiError> {
        let key_pair = Self::change_config(&mut self.lock(), |config, inner| {
            config.wireguard_key_cache.need_registration(inner.set_keychain_wg_sk.as_ref())
        })?;
        let Some((current_public_key, old_public_keys)) = key_pair else {
            tracing::info!("public wireguard key already registered");
            return Ok(());
        };
        let cmd = CacheWgKey {
            public_key: WgPubkey(current_public_key.to_bytes()),
            previous_public_keys: old_public_keys.iter().map(|p| WgPubkey(p.to_bytes())).collect(),
        };
        match self.api_request(cmd).await {
            Ok(()) => {
                Self::change_config(&mut self.lock(), |config, _| {
                    config.wireguard_key_cache.registered(current_public_key, &old_public_keys)
                })?;
                tracing::info!("successfully registered public wireguard key");
                Ok(())
            }
            Err(error) => {
                if matches!(error.api_error_kind(), Some(ApiErrorKind::WgKeyRotationRequired {})) {
                    tracing::warn!(?error, "server indicated that key rotation is required immediately");
                    Self::change_config(&mut self.lock(), |config, inner| {
                        config.wireguard_key_cache.rotate_now(inner.set_keychain_wg_sk.as_ref())
                    })?;
                }
                Err(error)
            }
        }
    }

    pub fn rotate_wg_key(&self) -> Result<(), ConfigSaveError> {
        Self::change_config(&mut self.lock(), |config, inner| {
            config.wireguard_key_cache.rotate_now(inner.set_keychain_wg_sk.as_ref());
        })
    }

    pub fn subscribe_exit_list(&self) -> tokio::sync::watch::Receiver<Option<ConfigCached<Arc<ExitList>>>> {
        self.exit_list_watch.subscribe()
    }
}
