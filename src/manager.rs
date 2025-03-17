use std::{
    ops::ControlFlow,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use futures::FutureExt;
use obscuravpn_api::{
    cmd::{Cmd, GetAccountInfo, ListExits2},
    types::{AccountId, AccountInfo, OneExit, WgPubkey},
    Client, ClientError,
};
use serde::{Deserialize, Serialize};
use tokio::{
    runtime::Runtime,
    sync::watch::{channel, Receiver, Sender},
    task::JoinHandle,
    time::sleep,
};
use tokio_util::sync::{CancellationToken, DropGuard};
use uuid::Uuid;

use crate::{
    client_state::AccountStatus,
    config::{Config, ConfigLoadError, ConfigSaveError, PinnedLocation},
    errors::ApiError,
    quicwg::{QuicWgConn, QuicWgTrafficStats},
};

use crate::{client_state::ClientState, errors::ConnectErrorCode, network_config::NetworkConfig};

use super::ffi_helpers::*;

pub struct Manager {
    // When we implement support for an OS, which doesn't need to fall back to a C API, we may want to move these callbacks into OS specific code and use some trait or returned stream for received packets and a status stream for network config and tunnel status callbacks.
    receive_cb: extern "C" fn(FfiBytes),
    network_config_cb: extern "C" fn(FfiBytes),
    tunnel_status_cb: extern "C" fn(isConnected: bool),
    client_state: Arc<ClientState>,
    tunnel_state: Arc<RwLock<TunnelState>>,
    status_watch: Sender<Status>,
    _background_task_cancellation: DropGuard,
}

// Keep synchronized with ../../apple/shared/NetworkExtensionIpc.swift
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
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
}

impl Status {
    fn new(version: Uuid, vpn_status: VpnStatus, config: Config, api_url: String) -> Self {
        let Config { account_id, in_new_account_flow, pinned_locations, last_chosen_exit, cached_account_status, .. } = config;
        Self {
            version,
            vpn_status,
            account_id,
            in_new_account_flow,
            pinned_locations: pinned_locations.unwrap_or_default(),
            last_chosen_exit,
            api_url,
            account: cached_account_status,
        }
    }
}

// Keep synchronized with ../../apple/shared/NetworkExtensionIpc.swift
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum VpnStatus {
    Connecting {},
    Connected {
        exit: OneExit,
        client_public_key: WgPubkey,
        exit_public_key: WgPubkey,
    },
    Reconnecting {
        exit: OneExit,
        // TODO: Maybe add disconnect reason?
        err: Option<ConnectErrorCode>,
    },
    Disconnected {},
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TunnelArgs {
    exit: Option<String>,
}

#[allow(clippy::large_enum_variant)]
pub enum TunnelState {
    Disconnected {
        conn_id: Uuid,
    },
    Starting {
        conn_id: Uuid,
    },
    Running {
        started_at: Instant,
        conn_id: Uuid,
        conn: Arc<QuicWgConn>,
        run_task: JoinHandle<()>,
        reconnecting: bool,
        reconnect_err: Option<ConnectErrorCode>,
        traffic_stats_offset: QuicWgTrafficStats,
        exit: OneExit,
    },
}

impl TunnelState {
    fn vpn_status(&self) -> VpnStatus {
        match self {
            TunnelState::Disconnected { .. } => VpnStatus::Disconnected {},
            TunnelState::Starting { .. } => VpnStatus::Connecting {},
            TunnelState::Running { conn, reconnecting, reconnect_err, exit, .. } => match reconnecting {
                true => VpnStatus::Reconnecting { err: *reconnect_err, exit: exit.clone() },
                false => VpnStatus::Connected {
                    exit: exit.clone(),
                    client_public_key: WgPubkey(conn.client_public_key().to_bytes()),
                    exit_public_key: WgPubkey(conn.exit_public_key().to_bytes()),
                },
            },
        }
    }
}

impl Manager {
    pub fn new(
        config_dir: PathBuf,
        old_config_dir: PathBuf,
        user_agent: String,
        runtime: &Runtime,
        receive_cb: extern "C" fn(FfiBytes),
        network_config_cb: extern "C" fn(FfiBytes),
        tunnel_status_cb: extern "C" fn(isConnected: bool),
    ) -> Result<Arc<Self>, ConfigLoadError> {
        let client_state = ClientState::new(config_dir, old_config_dir, user_agent)?;
        let config = client_state.get_config();
        let initial_status = Status::new(Uuid::new_v4(), VpnStatus::Disconnected {}, config, client_state.base_url());
        let background_task_cancellation = CancellationToken::new();
        let this: Arc<Self> = Self {
            receive_cb,
            network_config_cb,
            tunnel_status_cb,
            client_state: client_state.into(),
            tunnel_state: Arc::new(RwLock::new(TunnelState::Disconnected { conn_id: Uuid::new_v4() })),
            status_watch: channel(initial_status).0,
            _background_task_cancellation: background_task_cancellation.clone().drop_guard(),
        }
        .into();
        let background_fut = this.clone().wireguard_key_registraction_task();
        runtime.spawn(async move {
            background_task_cancellation.run_until_cancelled(background_fut).await;
        });
        Ok(this)
    }

    // Non-async exclusive mutable access to tunnel state
    fn write_tunnel_state<T>(&self, f: impl FnOnce(&mut TunnelState) -> T) -> T {
        let mut tunnel_state = match self.tunnel_state.write() {
            Ok(t) => t,
            Err(err) => {
                tracing::error!("tunnel state mutex was poisoned");
                err.into_inner()
            }
        };
        let ret = f(&mut tunnel_state);
        self.update_status_if_changed(Some(tunnel_state.vpn_status()));
        ret
    }

    // Non-async non-exclusive immutable access to tunnel state
    fn read_tunnel_state<T>(&self, f: impl FnOnce(&TunnelState) -> T) -> T {
        let tunnel_state = match self.tunnel_state.read() {
            Ok(t) => t,
            Err(err) => {
                tracing::error!("tunnel state mutex was poisoned");
                err.into_inner()
            }
        };
        f(&tunnel_state)
    }

    pub fn subscribe(&self) -> Receiver<Status> {
        self.status_watch.subscribe()
    }

    // TODO: Maybe specific error type, which absorbs or can be convertedd to high-level error code. Wait how the handling of unexpected tunnel states evolves.
    pub async fn start(self: Arc<Self>, args: TunnelArgs) -> Result<NetworkConfig, ConnectErrorCode> {
        tracing::info!(
            args =? args,
            "Starting tunnel."
        );

        let conn_id = self.write_tunnel_state(|tunnel_state| {
            let conn_id = match &*tunnel_state {
                TunnelState::Disconnected { conn_id } => Some(*conn_id),
                TunnelState::Starting { conn_id: prev_conn_id, .. } => {
                    tracing::warn!(%prev_conn_id, "TunnelState already Starting");
                    None
                }
                TunnelState::Running { conn_id: prev_conn_id, run_task, .. } => {
                    tracing::warn!(%prev_conn_id, "TunnelState already Running");
                    run_task.abort();
                    None
                }
            };

            let conn_id = conn_id.unwrap_or_else(Uuid::new_v4);
            *tunnel_state = TunnelState::Starting { conn_id };
            tracing::info!(%conn_id, "setting TunnelState to Starting");
            conn_id
        });

        let connect_result = self.client_state.connect(args.exit.clone()).await;
        if let Err(err) = &connect_result {
            tracing::error!(%conn_id, %err, "could not connect");
        }

        self.write_tunnel_state(|tunnel_state| match tunnel_state {
            TunnelState::Disconnected { conn_id: other_conn_id } => {
                tracing::warn!(%conn_id, %other_conn_id, "TunnelState was set to Disconnected before completing connection attempt");
                Err(ConnectErrorCode::Other)
            }
            TunnelState::Starting { conn_id: other_conn_id, .. } if *other_conn_id != conn_id => {
                tracing::warn!(%conn_id, %other_conn_id, "TunnelState connection id does not match");
                Err(ConnectErrorCode::Other)
            }
            TunnelState::Running { conn_id: other_conn_id, .. } => {
                tracing::warn!(%conn_id, %other_conn_id, "TunnelState was set to Running before completing connection attempt");
                Err(ConnectErrorCode::Other)
            }
            TunnelState::Starting { conn_id: _, .. } => match connect_result {
                Ok((conn, net_config, exit, _relay)) => {
                    tracing::info!(%conn_id, "setting TunnelState to Running");
                    let conn: Arc<QuicWgConn> = Arc::new(conn);
                    let run_task = tokio::spawn(run_tunnel(self.clone(), conn.clone(), args, conn_id).map(|_| ()));
                    *tunnel_state = TunnelState::Running {
                        started_at: Instant::now(),
                        conn_id,
                        conn,
                        run_task,
                        reconnect_err: None,
                        reconnecting: false,
                        traffic_stats_offset: Default::default(),
                        exit,
                    };
                    Ok(net_config)
                }
                Err(err) => {
                    let new_conn_id = Uuid::new_v4();
                    tracing::info!(%conn_id, %new_conn_id,"setting TunnelState to Disconnected due to error");
                    *tunnel_state = TunnelState::Disconnected { conn_id: new_conn_id };
                    Err(ConnectErrorCode::from(&err))
                }
            },
        })
    }

    pub fn send_packet(&self, packet: &[u8]) {
        self.read_tunnel_state(|tunnel_state| {
            if let TunnelState::Running { conn_id: _, conn, run_task: _, .. } = tunnel_state {
                conn.send(packet);
            }
        })
    }

    pub fn stop(&self) {
        self.write_tunnel_state(|tunnel_state: &mut TunnelState| {
            match tunnel_state {
                TunnelState::Disconnected { conn_id } => tracing::info!(%conn_id, "TunnelState already Disconnected"),
                TunnelState::Starting { conn_id } => tracing::warn!(%conn_id, "stopping from TunnelState Starting"),
                TunnelState::Running { conn_id, conn: _, run_task, .. } => {
                    tracing::warn!(%conn_id, "aborting TunnelState Running");
                    run_task.abort();
                }
            }
            let new_conn_id = Uuid::new_v4();
            tracing::info!(%new_conn_id, "setting TunnelState to Disconnected");
            *tunnel_state = TunnelState::Disconnected { conn_id: new_conn_id };
            if let Err(error) = self.client_state.rotate_wireguard_key_if_required() {
                tracing::error!(?error, "wireguard key rotation after stopping tunnel failed")
            }
        })
    }

    pub fn traffic_stats(&self) -> ManagerTrafficStats {
        let (conn_id, stats, offset, connected) = self.read_tunnel_state(|tunnel_state| match tunnel_state {
            TunnelState::Disconnected { conn_id } | TunnelState::Starting { conn_id } => {
                (*conn_id, Default::default(), Default::default(), Duration::ZERO)
            }
            TunnelState::Running { conn_id, conn, traffic_stats_offset, started_at, .. } => {
                (*conn_id, conn.traffic_stats(), *traffic_stats_offset, started_at.elapsed())
            }
        });
        ManagerTrafficStats {
            connected_ms: connected.as_millis() as u64,
            conn_id,
            tx_bytes: offset.tx_bytes + stats.tx_bytes,
            rx_bytes: offset.rx_bytes + stats.rx_bytes,
            latest_latency_ms: stats.latest_latency_ms,
        }
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

    pub async fn list_exits(&self) -> Result<obscuravpn_api::cmd::ExitList, ApiError> {
        let exits = self.api_request(ListExits2 {}).await?;
        self.client_state.maybe_migrate_pinned_exits(&exits)?;
        Ok(exits)
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

    async fn wireguard_key_registraction_task(self: Arc<Self>) {
        let mut status_subscription = self.subscribe();
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
                    tracing::error!("status subscription closed unexpectedly");
                    return;
                };
                last_status_version = Some(status.version);
            }
            for backoff_wait in 0..10 {
                let Err(error) = self.client_state.register_cached_wireguard_key_if_new().await else {
                    continue;
                };
                tracing::warn!(?error, "failed attempt to register cached wireguard key");
                sleep(Duration::from_secs(backoff_wait)).await;
            }
        }
    }

    pub fn rotate_wg_key(&self) -> Result<(), ConfigSaveError> {
        self.client_state.rotate_wg_key()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagerTrafficStats {
    connected_ms: u64,
    conn_id: Uuid,
    tx_bytes: u64,
    rx_bytes: u64,
    latest_latency_ms: u16,
}

async fn run_tunnel(manager: Arc<Manager>, mut conn: Arc<QuicWgConn>, args: TunnelArgs, conn_id: Uuid) -> ControlFlow<()> {
    loop {
        match conn.receive().await {
            Ok(packet) => (manager.receive_cb)(packet.ffi()),
            Err(err) => {
                tracing::error!(%conn_id, %err, "tunnel failed, reconnecting");
                manager.write_tunnel_state(|tunnel_state| match tunnel_state {
                    TunnelState::Disconnected { conn_id: other_conn_id } => {
                        tracing::error!(%conn_id, %other_conn_id, "failed, but TunnelState is Disconnected");
                        ControlFlow::Break(())
                    }
                    TunnelState::Starting { conn_id: other_conn_id } => {
                        tracing::error!(%conn_id, %other_conn_id, "failed, but TunnelState is Starting");
                        ControlFlow::Break(())
                    }
                    TunnelState::Running { conn_id: other_conn_id, .. } if conn_id != *other_conn_id => {
                        tracing::error!(%conn_id, %other_conn_id, "failed, but conn_id does not match");
                        ControlFlow::Break(())
                    }
                    TunnelState::Running { reconnecting, .. } => {
                        tracing::info!(%conn_id, "setting TunnelState reconnecting flag to true");
                        *reconnecting = true;
                        ControlFlow::Continue(())
                    }
                })?;
                (manager.tunnel_status_cb)(false);

                let (new_conn, net_config, new_exit, _new_relay) = loop {
                    sleep(Duration::from_secs(1)).await;
                    match manager.client_state.connect(args.exit.clone()).await {
                        Ok(ok) => break ok,
                        Err(err) => {
                            tracing::error!(%conn_id, %err, "tunnel reconnect attempt failed");
                            manager.write_tunnel_state(|tunnel_state| match tunnel_state {
                                TunnelState::Disconnected { conn_id: other_conn_id } => {
                                    tracing::error!(%conn_id, %other_conn_id, "reconnect failed, but TunnelState is Disconnected");
                                    ControlFlow::Break(())
                                }
                                TunnelState::Starting { conn_id: other_conn_id } => {
                                    tracing::error!(%conn_id, %other_conn_id, "reconnect failed, but TunnelState is Starting");
                                    ControlFlow::Break(())
                                }
                                TunnelState::Running { conn_id: other_conn_id, .. } if conn_id != *other_conn_id => {
                                    tracing::error!(%conn_id, %other_conn_id, "reconnect failed, but conn_id does not match");
                                    ControlFlow::Break(())
                                }
                                TunnelState::Running { reconnect_err, .. } => {
                                    tracing::info!(%conn_id, "setting TunnelState reconnect error");
                                    *reconnect_err = Some((&err).into());
                                    ControlFlow::Continue(())
                                }
                            })?;
                            (manager.tunnel_status_cb)(false);
                        }
                    }
                };
                tracing::info!(%conn_id, "tunnel reconnect attempt succeeded");
                conn = Arc::new(new_conn);
                manager.write_tunnel_state(|tunnel_state| match tunnel_state {
                    TunnelState::Disconnected { conn_id: other_conn_id } => {
                        tracing::error!(%conn_id, %other_conn_id, "reconnected, but TunnelState is Disconnected");
                        ControlFlow::Break(())
                    }
                    TunnelState::Starting { conn_id: other_conn_id } => {
                        tracing::error!(%conn_id, %other_conn_id, "reconnected, but TunnelState is Starting");
                        ControlFlow::Break(())
                    }
                    TunnelState::Running { conn_id: other_conn_id, .. } if conn_id != *other_conn_id => {
                        tracing::error!(%conn_id, %other_conn_id, "reconnected, but conn_id does not match");
                        ControlFlow::Break(())
                    }
                    TunnelState::Running { conn_id: _, conn: failed_conn, reconnect_err, reconnecting, traffic_stats_offset, exit, .. } => {
                        tracing::info!(%conn_id, "updating TunnelState connection handle");
                        *reconnect_err = None;
                        *reconnecting = false;
                        *exit = new_exit;
                        let failed_conn_traffic_stats = failed_conn.traffic_stats();
                        traffic_stats_offset.tx_bytes += failed_conn_traffic_stats.tx_bytes;
                        traffic_stats_offset.rx_bytes += failed_conn_traffic_stats.rx_bytes;
                        *failed_conn = conn.clone();
                        ControlFlow::Continue(())
                    }
                })?;
                (manager.tunnel_status_cb)(true);
                (manager.network_config_cb)(serde_json::to_vec(&net_config).unwrap().ffi());
            }
        }
    }
}
