use std::{path::PathBuf, sync::Arc, time::Duration};

use obscuravpn_api::cmd::ExitList;
use obscuravpn_api::{
    cmd::{AppleAssociateAccount, AppleAssociateAccountOutput, Cmd, DeleteAccount, DeleteAccountOutput, GetAccountInfo},
    types::{AccountId, AccountInfo, OneExit, OneRelay, WgPubkey},
};
use serde::{Deserialize, Serialize};
use tokio::select;
use tokio::sync::watch::{Receiver, Sender, channel};
use uuid::Uuid;

use crate::client_state::ClientStateHandle;
use crate::errors::{ConfigDirty, ConfigDirtyOrApiError};
use crate::manager_cmd::{ManagerCmdErrorCode, ManagerCmdOk};
use crate::os::os_trait::Os;
use crate::{
    backoff::Backoff,
    client_state::{AccountStatus, ClientState},
    config::{Config, ConfigLoadError, KeychainSetSecretKeyFn, PinnedLocation, feature_flags::FeatureFlags},
    debug_archive::create_debug_archive,
    errors::{ApiError, ConnectErrorCode},
    exit_selection::ExitSelector,
    logging::LogPersistence,
    net::NetworkInterface,
    network_config::DnsContentBlock,
    quicwg::TransportKind,
    tunnel_state::TunnelState,
};
use crate::{cached_value::CachedValue, debug_archive::info::DebugInfo};

pub struct Manager {
    client_state: ClientStateHandle,
    tunnel_state: Receiver<TunnelState>,
    status_watch: Sender<Status>,
    log_persistence: Option<Box<LogPersistence>>,
}

// Keep synchronized with ../../apple/shared/NetworkExtensionIpc.swift
#[derive(Debug, Serialize, PartialEq, Eq, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub version: Uuid,
    pub vpn_status: VpnStatus,
    pub account_id: Option<AccountId>,
    pub in_new_account_flow: bool,
    pub pinned_locations: Vec<PinnedLocation>,
    pub last_chosen_exit: ExitSelector,
    pub last_exit: ExitSelector,
    pub api_url: String,
    pub account: Option<AccountStatus>,
    pub auto_connect: bool,
    pub feature_flags: FeatureFlags,
    pub feature_flag_keys: Vec<String>,
    pub use_system_dns: bool,
    pub dns_content_block: DnsContentBlock,
}

impl Status {
    fn new(version: Uuid, vpn_status: VpnStatus, client_state: &ClientState) -> Self {
        let Config {
            account_id,
            in_new_account_flow,
            pinned_locations,
            last_chosen_exit_selector,
            last_exit_selector,
            cached_account_status,
            auto_connect,
            feature_flags,
            dns,
            dns_content_block,
            ..
        } = client_state.config();
        let api_url = client_state.base_url();
        Self {
            version,
            vpn_status,
            account_id: account_id.clone(),
            in_new_account_flow: *in_new_account_flow,
            pinned_locations: pinned_locations.clone(),
            last_chosen_exit: last_chosen_exit_selector.clone(),
            last_exit: last_exit_selector.clone(),
            api_url,
            account: cached_account_status.clone(),
            auto_connect: *auto_connect,
            feature_flags: feature_flags.clone(),
            feature_flag_keys: FeatureFlags::KEYS.iter().map(ToString::to_string).collect(),
            use_system_dns: dns.is_system(),
            dns_content_block: *dns_content_block,
        }
    }
}

// Keep synchronized with ../../apple/shared/NetworkExtensionIpc.swift
#[derive(Debug, Serialize, PartialEq, Eq, Clone, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum VpnStatus {
    Connecting {
        tunnel_args: TunnelArgs,
        connect_error: Option<ConnectErrorCode>,
        reconnecting: bool,
    },
    Connected {
        tunnel_args: TunnelArgs,
        exit: OneExit,
        relay: OneRelay,
        client_public_key: WgPubkey,
        exit_public_key: WgPubkey,
        transport: TransportKind,
    },
    Disconnected {},
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TunnelArgs {
    pub exit: ExitSelector,
}

impl VpnStatus {
    fn from_tunnel_state(tunnel_state: &TunnelState) -> Self {
        match tunnel_state {
            TunnelState::Disconnected => VpnStatus::Disconnected {},
            TunnelState::Connecting { args, connect_error, disconnect_reason, offset_traffic_stats: _, network_interface: _ } => {
                VpnStatus::Connecting {
                    tunnel_args: args.clone(),
                    connect_error: connect_error.as_ref().map(|error_at| ConnectErrorCode::from(&error_at.error)),
                    reconnecting: disconnect_reason.is_some(),
                }
            }
            TunnelState::Connected { args, conn, relay, exit, network_config: _, offset_traffic_stats: _, network_interface: _ } => {
                VpnStatus::Connected {
                    tunnel_args: args.clone(),
                    relay: relay.clone(),
                    exit: exit.clone(),
                    client_public_key: WgPubkey(conn.client_public_key().to_bytes()),
                    exit_public_key: WgPubkey(conn.exit_public_key().to_bytes()),
                    transport: conn.transport(),
                }
            }
        }
    }
}

impl Manager {
    /// The constructed `Arc<Manager>` can not be dropped due to spawned tasks, which hold references.
    pub fn new(
        config_dir: PathBuf,
        keychain_wg_sk: Option<&[u8]>,
        user_agent: String,
        os_impl: Arc<impl Os>,
        set_keychain_wg_sk: Option<KeychainSetSecretKeyFn>,
        log_persistence: Option<Box<LogPersistence>>,
        force_init_inactive: bool,
    ) -> Result<Arc<Self>, ConfigLoadError> {
        let client_state = ClientState::new(config_dir, keychain_wg_sk, user_agent, set_keychain_wg_sk, force_init_inactive)?;
        let tunnel_state = TunnelState::new(client_state.clone(), os_impl.clone());
        let initial_status = Status::new(Uuid::new_v4(), VpnStatus::Disconnected {}, &client_state.borrow());
        let this = Arc::new(Self { tunnel_state, client_state, status_watch: channel(initial_status).0, log_persistence });
        tokio::spawn(Self::wireguard_key_registraction_task(this.clone(), ()));
        tokio::spawn(Self::propagate_updates_to_status_task(this.clone(), ()));
        tokio::spawn(Self::preferred_network_interface_task(this.clone(), os_impl.network_interface()));
        Ok(this)
    }

    pub async fn maybe_update_exits(&self, freshness: Duration) -> Result<(), ApiError> {
        self.client_state.maybe_update_exits(freshness).await
    }

    pub fn subscribe(&self) -> Receiver<Status> {
        self.status_watch.subscribe()
    }

    pub fn packets_for_relay<'a>(&self, packets: impl Iterator<Item = &'a [u8]>) {
        if let Some(conn) = self.tunnel_state.borrow().get_conn() {
            conn.send(packets);
        }
    }

    pub fn traffic_stats(&self) -> ManagerTrafficStats {
        self.tunnel_state.borrow().traffic_stats()
    }

    pub async fn login(&self, account_id: AccountId, validate: bool) -> Result<(), ConfigDirtyOrApiError> {
        let mut auth_token = None;
        if validate {
            const MAX_ATTEMPTS: usize = 10;
            for _ in 0..MAX_ATTEMPTS {
                let api_client = self.client_state.make_api_client(account_id.clone())?;
                let output = api_client.acquire_auth_token().await.map_err(ApiError::from)?;
                if let Some(url_override) = output.url_override {
                    // TODO: https://linear.app/soveng/issue/OBS-2268/override-web-url-for-apple-demo-accounts
                    self.set_api_url(Some(url_override.api));
                } else {
                    auth_token = Some(output.auth_token.into());
                    break;
                }
            }
            if auth_token.is_none() {
                return Err(ApiError::ApiClient(anyhow::format_err!("exceeded {MAX_ATTEMPTS} URL overrides").into()).into());
            }
        }
        self.client_state.set_account_id(Some((account_id, auth_token)))?;
        Ok(())
    }

    pub fn logout(&self) -> Result<(), ConfigDirty> {
        self.client_state.set_account_id(None)
    }

    pub fn set_api_url(&self, value: Option<String>) {
        self.client_state.set_api_url(value);
    }

    pub async fn api_request<C: Cmd>(&self, cmd: C) -> Result<C::Output, ApiError> {
        self.client_state.api_request(cmd).await
    }

    pub async fn apple_associate_account(&self, app_transaction_jws: String) -> Result<AppleAssociateAccountOutput, ApiError> {
        self.api_request(AppleAssociateAccount { app_transaction_jws }).await
    }

    pub async fn delete_account(&self) -> Result<DeleteAccountOutput, ApiError> {
        self.api_request(DeleteAccount {}).await
    }

    pub async fn get_account_info(&self) -> Result<AccountInfo, ApiError> {
        let account_info = self.api_request(GetAccountInfo()).await?;
        self.client_state.update_account_info(&account_info);
        Ok(account_info)
    }

    async fn propagate_updates_to_status_task(this: Arc<Self>, _: ()) {
        let mut tunnel_state_recv = this.tunnel_state.clone();
        let mut client_state_recv = this.client_state.subscribe();
        tunnel_state_recv.mark_changed();
        loop {
            let cont = select! {
                res = tunnel_state_recv.changed() => res.is_ok(),
                res = client_state_recv.changed() => res.is_ok(),
            };
            if !cont {
                break;
            };
            this.status_watch.send_if_modified(|status| {
                let vpn_status = VpnStatus::from_tunnel_state(&tunnel_state_recv.borrow_and_update());
                let client_state = client_state_recv.borrow_and_update();
                let mut new_status = Status::new(status.version, vpn_status, &client_state);
                if new_status == *status {
                    return false;
                }
                new_status.version = Uuid::new_v4();
                *status = new_status;
                true
            });
        }
        tracing::info!(message_id = "NUeloeKe", "propagate_updates_to_status_task stops")
    }

    async fn wireguard_key_registraction_task(this: Arc<Self>, _: ()) {
        let mut status_subscription = this.subscribe();
        let mut last_status_version = None;
        loop {
            {
                let status_result = status_subscription
                    .wait_for(|status| {
                        let changed = Some(status.version) != last_status_version;
                        let active = status.account.as_ref().is_some_and(|account_status| account_status.account_info.active);
                        let disconnected = matches!(status.vpn_status, VpnStatus::Disconnected {});
                        changed && active && disconnected
                    })
                    .await;
                let Ok(status) = status_result else {
                    tracing::info!("status subscription closed unexpectedly");
                    return;
                };
                last_status_version = Some(status.version);
            }
            let mut backoff = Backoff::BACKGROUND.take(10);
            while backoff.wait().await {
                let Err(error) = this.client_state.register_cached_wireguard_key_if_new().await else {
                    continue;
                };
                tracing::warn!(?error, "failed attempt to register cached wireguard key");
            }
        }
    }

    pub async fn preferred_network_interface_task(this: Arc<Self>, mut network_interface_watch: Receiver<Option<NetworkInterface>>) {
        loop {
            let preferred_network_interface = network_interface_watch.borrow_and_update().clone();
            this.client_state.set_network_interface(preferred_network_interface);
            if network_interface_watch.changed().await.is_err() {
                tracing::error!(message_id = "ybeBsPfE", "status subscription closed unexpectedly");
                return;
            }
        }
    }

    pub async fn create_debug_archive(&self, user_feedback: Option<&str>) -> anyhow::Result<String> {
        let user_feedback = user_feedback.map(ToOwned::to_owned);
        let log_dir = self.log_persistence.as_deref().map(LogPersistence::log_dir).map(ToOwned::to_owned);
        let debug_info = self.get_debug_info().await;
        tokio::task::spawn_blocking(move || create_debug_archive(user_feedback.as_deref(), debug_info, log_dir.as_deref()).map(Into::into)).await?
    }

    pub async fn get_debug_info(&self) -> DebugInfo {
        self.client_state.get_debug_info().await
    }

    pub fn wake(&self) {
        if let Some(conn) = self.tunnel_state.borrow().get_conn() {
            conn.wake();
        }
    }

    pub async fn get_exit_list(&self, known_version: Option<Vec<u8>>) -> Result<CachedValue<Arc<ExitList>>, ManagerCmdErrorCode> {
        let mut watch = self.client_state.subscribe();
        let client_state = watch
            .wait_for(|client_state| {
                client_state
                    .config()
                    .cached_exits
                    .clone()
                    .is_some_and(|e| Some(e.version()) != known_version.as_deref())
            })
            .await
            .map_err(|error| {
                tracing::error!(?error, message_id = "ahcieM1h", "exit list subscription channel closed: {}", error,);
                ManagerCmdErrorCode::Other
            })?;
        let cached = client_state.config().cached_exits.clone().unwrap();
        Ok(CachedValue { version: cached.version().to_vec(), last_updated: cached.last_updated, value: cached.value.clone() })
    }

    pub fn run_on_client_state(&self, f: impl FnOnce(&ClientStateHandle)) -> Result<ManagerCmdOk, ManagerCmdErrorCode> {
        f(&self.client_state);
        Ok(ManagerCmdOk::Empty)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagerTrafficStats {
    pub connected_ms: u64,
    pub conn_id: Uuid,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub latest_latency_ms: u16,
}
