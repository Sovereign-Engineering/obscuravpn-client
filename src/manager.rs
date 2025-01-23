use std::{
    ops::ControlFlow,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use futures::FutureExt;
use obscuravpn_api::{
    cmd::{Cmd, GetAccountInfo, ListExits2},
    types::{AccountInfo, OneExit, WgPubkey},
    Client, ClientError,
};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::watch::{channel, Receiver, Sender},
    task::JoinHandle,
    time::sleep,
};
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
    client_state: Arc<ClientState>,
    tunnel_state: Arc<RwLock<TunnelState>>,
    status_watch: Sender<Status>,
    started_at: Instant,
}

// Keep synchronized with ../../apple/shared/NetworkExtensionIpc.swift
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub version: Uuid,
    pub vpn_status: VpnStatus,
    pub account_id: Option<String>,
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
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
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

pub enum TunnelState {
    Disconnected {
        conn_id: Uuid,
    },
    Starting {
        conn_id: Uuid,
    },
    Running {
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
    pub fn new(config_dir: PathBuf, old_config_dir: PathBuf, user_agent: String) -> Result<Arc<Self>, ConfigLoadError> {
        let client_state = ClientState::new(config_dir, old_config_dir, user_agent)?;
        let config = client_state.get_config();
        let initial_status = Status::new(Uuid::new_v4(), VpnStatus::Disconnected {}, config, client_state.base_url());
        Ok(Self {
            client_state: client_state.into(),
            tunnel_state: Arc::new(RwLock::new(TunnelState::Disconnected { conn_id: Uuid::new_v4() })),
            started_at: Instant::now(),
            status_watch: channel(initial_status).0,
        }
        .into())
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
    // TODO: Once we have more than one user we want to remove `receive_cb` from this method. Eg. use a trait, drive the future from the caller or return packet stream.
    // TODO: Remove network config and tunnel status callbacks in favor of proper status stream
    pub async fn start(
        self: Arc<Self>,
        args: TunnelArgs,
        receive_cb: extern "C" fn(FfiBytes),
        network_config_cb: extern "C" fn(FfiBytes),
        tunnel_status_cb: extern "C" fn(isConnected: bool),
    ) -> Result<NetworkConfig, ConnectErrorCode> {
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
                    let run_task = tokio::spawn(
                        run_tunnel(self.clone(), conn.clone(), args, conn_id, receive_cb, network_config_cb, tunnel_status_cb).map(|_| ()),
                    );
                    *tunnel_state = TunnelState::Running {
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
                // TODO: log errors (or remove return value), but rate-limit to a few per minute, because this can get VERY noisy and is usually not interesting
                _ = conn.send(packet);
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
        })
    }

    pub fn traffic_stats(&self) -> ManagerTrafficStats {
        let (conn_id, stats, offset) = self.read_tunnel_state(|tunnel_state| match tunnel_state {
            TunnelState::Disconnected { conn_id } | TunnelState::Starting { conn_id } => (*conn_id, Default::default(), Default::default()),
            TunnelState::Running { conn_id, conn, traffic_stats_offset, .. } => (*conn_id, conn.traffic_stats(), *traffic_stats_offset),
        });
        ManagerTrafficStats {
            timestamp_ms: self.started_at.elapsed().as_millis() as u64,
            conn_id,
            tx_bytes: offset.tx_bytes + stats.tx_bytes,
            rx_bytes: offset.rx_bytes + stats.rx_bytes,
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

    pub async fn login(&self, account_id: String, validate: bool) -> Result<(), ApiError> {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagerTrafficStats {
    timestamp_ms: u64,
    conn_id: Uuid,
    tx_bytes: u64,
    rx_bytes: u64,
}

async fn run_tunnel(
    manager: Arc<Manager>,
    mut conn: Arc<QuicWgConn>,
    args: TunnelArgs,
    conn_id: Uuid,
    receive_cb: extern "C" fn(FfiBytes),
    network_config_cb: extern "C" fn(FfiBytes),
    tunnel_status_cb: extern "C" fn(isConnected: bool),
) -> ControlFlow<()> {
    let mut buf = vec![0u8; u16::MAX as usize];
    loop {
        match conn.receive(&mut buf).await {
            Ok(packet) => receive_cb(packet.ffi()),
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
                tunnel_status_cb(false);

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
                            tunnel_status_cb(false);
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
                tunnel_status_cb(true);
                network_config_cb(serde_json::to_vec(&net_config).unwrap().ffi());
            }
        }
    }
}
