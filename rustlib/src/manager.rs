use std::{
    future::Future,
    path::PathBuf,
    sync::{Arc, Weak},
    time::Duration,
};

use obscuravpn_api::{
    cmd::{Cmd, ExitList, GetAccountInfo},
    types::{AccountId, AccountInfo, OneExit, OneRelay, WgPubkey},
    Client, ClientError,
};
use serde::{Deserialize, Serialize};
use tokio::{
    runtime::{Handle, Runtime},
    sync::watch::{channel, Receiver, Sender},
    time::sleep,
};
use tokio_util::sync::{CancellationToken, DropGuard};
use uuid::Uuid;

use crate::{
    client_state::AccountStatus,
    config::{cached::ConfigCached, Config, ConfigDebug, ConfigLoadError, ConfigSaveError, PinnedLocation},
    errors::ApiError,
    tunnel_state::TunnelState,
};

use crate::{client_state::ClientState, errors::ConnectErrorCode, network_config::NetworkConfig};

use super::ffi_helpers::*;

pub struct Manager {
    background_taks_cancellation_token: CancellationToken,
    client_state: Arc<ClientState>,
    tunnel_state: Receiver<TunnelState>,
    target_tunnel_args: Sender<Option<TunnelArgs>>,
    status_watch: Sender<Status>,
    runtime: Handle,
    _background_task_drop_guard: DropGuard,
}

// Keep synchronized with ../../apple/shared/NetworkExtensionIpc.swift
#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub version: Uuid,
    pub vpn_status: VpnStatus,
    pub account_id: Option<AccountId>,
    pub in_new_account_flow: bool,
    pub pinned_locations: Vec<PinnedLocation>,
    pub last_chosen_exit: Option<String>,
    pub api_url: String,
    pub account: Option<AccountStatus>,
    pub auto_connect: bool,
}

impl Status {
    fn new(version: Uuid, vpn_status: VpnStatus, config: Config, api_url: String) -> Self {
        let Config {
            account_id,
            in_new_account_flow,
            pinned_locations,
            last_chosen_exit,
            cached_account_status,
            auto_connect,
            ..
        } = config;
        Self {
            version,
            vpn_status,
            account_id,
            in_new_account_flow,
            pinned_locations,
            last_chosen_exit,
            api_url,
            account: cached_account_status,
            auto_connect,
        }
    }
}

// Keep synchronized with ../../apple/shared/NetworkExtensionIpc.swift
#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
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
        network_config: NetworkConfig,
        client_public_key: WgPubkey,
        exit_public_key: WgPubkey,
    },
    Disconnected {},
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TunnelArgs {
    pub exit: ExitSelector,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ExitSelector {
    Any {},
    Exit { id: String },
    Country { country_code: String },
    City { country_code: String, city_code: String },
}

impl Default for ExitSelector {
    fn default() -> Self {
        ExitSelector::Any {}
    }
}

impl VpnStatus {
    fn from_tunnel_state(tunnel_state: &TunnelState) -> Self {
        match tunnel_state {
            TunnelState::Disconnected { .. } => VpnStatus::Disconnected {},
            TunnelState::Connecting { args, connect_error, disconnect_reason, offset_traffic_stats: _ } => VpnStatus::Connecting {
                tunnel_args: args.clone(),
                connect_error: connect_error.as_ref().map(|error_at| ConnectErrorCode::from(&error_at.error)),
                reconnecting: disconnect_reason.is_some(),
            },
            TunnelState::Connected { args, conn, relay, exit, network_config, offset_traffic_stats: _ } => VpnStatus::Connected {
                tunnel_args: args.clone(),
                relay: relay.clone(),
                exit: exit.clone(),
                network_config: network_config.clone(),
                client_public_key: WgPubkey(conn.client_public_key().to_bytes()),
                exit_public_key: WgPubkey(conn.exit_public_key().to_bytes()),
            },
        }
    }
}

impl Manager {
    pub fn new(
        config_dir: PathBuf,
        user_agent: String,
        runtime: &Runtime,
        receive_cb: extern "C" fn(FfiBytes),
    ) -> Result<Arc<Self>, ConfigLoadError> {
        let cancellation_token = CancellationToken::new();
        let client_state = Arc::new(ClientState::new(config_dir, user_agent)?);
        let config = client_state.get_config();
        let (target_tunnel_args, tunnel_state) = TunnelState::new(runtime, client_state.clone(), receive_cb, cancellation_token.clone());
        let initial_status = Status::new(Uuid::new_v4(), VpnStatus::Disconnected {}, config, client_state.base_url());
        let this = Arc::new(Self {
            target_tunnel_args,
            tunnel_state,
            client_state,
            status_watch: channel(initial_status).0,
            runtime: runtime.handle().clone(),
            _background_task_drop_guard: cancellation_token.clone().drop_guard(),
            background_taks_cancellation_token: cancellation_token,
        });
        this.spawn_child_task(Self::wireguard_key_registraction_task);
        this.spawn_child_task(Self::propagate_tunnel_state_updates_to_status_task);
        Ok(this)
    }

    pub async fn maybe_update_exits(&self, freshness: Duration) -> Result<(), ApiError> {
        self.client_state.maybe_update_exits(freshness).await
    }

    pub fn subscribe(&self) -> Receiver<Status> {
        self.status_watch.subscribe()
    }

    pub fn subscribe_exit_list(&self) -> Receiver<Option<ConfigCached<Arc<ExitList>>>> {
        self.client_state.subscribe_exit_list()
    }

    pub fn set_target_state(&self, new_target_args: Option<TunnelArgs>, allow_activation: bool) -> Result<(), ()> {
        let mut ret = Ok(());
        let ret_ref = &mut ret;
        _ = self.target_tunnel_args.send_if_modified(move |target_args| {
            if target_args == &new_target_args {
                tracing::warn!(
                    message_id = "oqQ8GZEE",
                    "not setting target state, because the new one is identical to the current one"
                );
                return false;
            }
            if target_args.is_none() && new_target_args.is_some() && !allow_activation {
                *ret_ref = Err(());
                tracing::warn!(message_id = "juurJ3bm", "not setting target state, because activation is not allowed");
                return false;
            }
            *target_args = new_target_args;
            true
        });
        ret
    }

    pub fn send_packet(&self, packet: &[u8]) {
        if let Some(conn) = self.tunnel_state.borrow().get_conn() {
            conn.send(packet);
        }
    }

    pub fn traffic_stats(&self) -> ManagerTrafficStats {
        self.tunnel_state.borrow().traffic_stats()
    }

    fn update_status_if_changed(&self, new_vpn_status: Option<VpnStatus>) {
        self.status_watch.send_if_modified(|status| {
            let config = self.client_state.get_config();
            let vpn_status = new_vpn_status.unwrap_or_else(|| status.vpn_status.clone());
            let mut new_status = Status::new(status.version, vpn_status, config, self.client_state.base_url());
            if new_status == *status {
                return false;
            }
            new_status.version = Uuid::new_v4();
            *status = new_status;
            true
        });
    }

    pub async fn login(&self, account_id: AccountId, validate: bool) -> Result<(), ApiError> {
        let auth_token = if validate {
            let api_client =
                Client::new(self.client_state.base_url(), account_id.clone(), self.client_state.user_agent()).map_err(ClientError::from)?;
            Some(api_client.acquire_auth_token().await?)
        } else {
            None
        };
        let ret = self.client_state.set_account_id(Some(account_id), auth_token);
        self.update_status_if_changed(None);
        ret.map_err(Into::into)
    }

    pub fn logout(&self) -> Result<(), ConfigSaveError> {
        let ret = self.client_state.set_account_id(None, None);
        self.update_status_if_changed(None);
        ret
    }

    pub fn set_pinned_exits(&self, exits: Vec<PinnedLocation>) -> Result<(), ConfigSaveError> {
        let ret = self.client_state.set_pinned_locations(exits);
        self.update_status_if_changed(None);
        ret
    }

    pub fn set_in_new_account_flow(&self, value: bool) -> Result<(), ConfigSaveError> {
        let ret = self.client_state.set_in_new_account_flow(value);
        self.update_status_if_changed(None);
        ret
    }

    pub fn set_api_url(&self, value: Option<String>) -> Result<(), ConfigSaveError> {
        let ret = self.client_state.set_api_url(value);
        self.update_status_if_changed(None);
        ret
    }

    pub async fn api_request<C: Cmd>(&self, cmd: C) -> Result<C::Output, ApiError> {
        self.client_state.api_request(cmd).await
    }

    pub async fn get_account_info(&self) -> Result<AccountInfo, ApiError> {
        let account_info = self.api_request(GetAccountInfo()).await?;
        self.client_state.update_account_info(&account_info)?;
        self.update_status_if_changed(None);
        Ok(account_info)
    }

    fn spawn_child_task<F>(self: &Arc<Self>, constructor: impl FnOnce(Weak<Self>) -> F)
    where
        F: Future<Output = ()> + Send + Sync + 'static,
    {
        let cancellation_token = self.background_taks_cancellation_token.clone();
        let this = Arc::downgrade(self);
        let task = constructor(this);
        self.runtime.spawn(async move {
            cancellation_token.run_until_cancelled(task).await;
        });
    }

    async fn propagate_tunnel_state_updates_to_status_task(this: Weak<Self>) {
        let mut tunnel_state_recv = {
            let Some(this) = this.upgrade() else {
                tracing::error!(
                    message_id = "rkWUIljV",
                    "could not start propagate_tunnel_state_updates_to_status_task task"
                );
                return;
            };
            this.tunnel_state.clone()
        };
        tunnel_state_recv.mark_changed();
        while let Ok(()) = tunnel_state_recv.changed().await {
            let new_tunnel_state_ref = tunnel_state_recv.borrow_and_update();
            let new_vpn_status = VpnStatus::from_tunnel_state(&new_tunnel_state_ref);
            drop(new_tunnel_state_ref);
            let Some(this) = this.upgrade() else { break };
            this.update_status_if_changed(Some(new_vpn_status));
        }
        tracing::info!(message_id = "NUeloeKe", "propagate_tunnel_state_updates_to_status_task stops")
    }

    async fn wireguard_key_registraction_task(this: Weak<Self>) {
        let mut status_subscription = {
            let Some(this) = this.upgrade() else {
                tracing::error!(message_id = "9UObgSBK", "could not start wireguard_key_registraction_task task");
                return;
            };
            this.subscribe()
        };
        let mut last_status_version = None;
        'outer: loop {
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
            for backoff_wait in 0..10 {
                let Some(this) = this.upgrade() else {
                    break 'outer;
                };
                let Err(error) = this.client_state.register_cached_wireguard_key_if_new().await else {
                    continue;
                };
                tracing::warn!(?error, "failed attempt to register cached wireguard key");
                drop(this);
                sleep(Duration::from_secs(backoff_wait)).await;
            }
        }
        tracing::info!(message_id = "RG0S8UvK", "wireguard_key_registraction_task stops");
    }

    pub fn rotate_wg_key(&self) -> Result<(), ConfigSaveError> {
        self.client_state.rotate_wg_key()
    }

    pub fn get_debug_info(&self) -> DebugInfo {
        DebugInfo { config: self.client_state.get_config().into() }
    }

    pub fn set_auto_connect(&self, enable: bool) -> Result<(), ConfigSaveError> {
        self.client_state.set_auto_connect(enable)?;
        self.update_status_if_changed(None);
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DebugInfo {
    config: ConfigDebug,
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
