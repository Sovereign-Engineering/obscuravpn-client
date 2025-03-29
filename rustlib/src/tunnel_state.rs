use std::{future::Future, sync::Arc};

use futures::future::pending;
use obscuravpn_api::types::{OneExit, OneRelay};
use strum::EnumIs;
use tokio::runtime::Runtime;
use tokio::select;
use tokio::sync::watch::{channel, Receiver, Sender};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::errors::{ErrorAt, TunnelConnectError};
use crate::ffi_helpers::{FfiBytes, FfiBytesExt};
use crate::manager::ManagerTrafficStats;
use crate::network_config::NetworkConfig;
use crate::quicwg::{QuicWgReceiveError, QuicWgTrafficStats};
use crate::{client_state::ClientState, manager::TunnelArgs, quicwg::QuicWgConn};

#[derive(derive_more::Debug, EnumIs)]
pub enum TunnelState {
    Disconnected,
    Connecting {
        args: TunnelArgs,
        connect_error: Option<ErrorAt<TunnelConnectError>>,
        disconnect_reason: Option<ErrorAt<QuicWgReceiveError>>,
        offset_traffic_stats: ManagerTrafficStats,
    },
    Connected {
        args: TunnelArgs,
        #[debug(skip)]
        conn: Arc<QuicWgConn>,
        network_config: NetworkConfig,
        relay: OneRelay,
        exit: OneExit,
        offset_traffic_stats: ManagerTrafficStats,
    },
}

impl TunnelState {
    pub fn new(
        runtime: &Runtime,
        client_state: Arc<ClientState>,
        receive_cb: extern "C" fn(FfiBytes),
        cancel: CancellationToken,
    ) -> (Sender<Option<TunnelArgs>>, Receiver<TunnelState>) {
        let (target_args_send, target_args_recv) = channel(None);
        let (tunnel_state_send, tunnel_state_recv) = channel(TunnelState::Disconnected);
        runtime.spawn(async move {
            cancel
                .run_until_cancelled(Self::maintain(target_args_recv, tunnel_state_send, client_state, receive_cb))
                .await;
        });
        (target_args_send, tunnel_state_recv)
    }

    pub fn traffic_stats(&self) -> ManagerTrafficStats {
        match self {
            TunnelState::Disconnected => {
                ManagerTrafficStats { connected_ms: 0, conn_id: Uuid::new_v4(), tx_bytes: 0, rx_bytes: 0, latest_latency_ms: 0 }
            }
            TunnelState::Connecting { offset_traffic_stats, .. } => *offset_traffic_stats,
            TunnelState::Connected { conn, offset_traffic_stats, .. } => {
                let mut traffic_stats = *offset_traffic_stats;
                let QuicWgTrafficStats { connected_at, tx_bytes, rx_bytes, latest_latency_ms } = conn.traffic_stats();
                traffic_stats.connected_ms = traffic_stats
                    .connected_ms
                    .saturating_add(connected_at.elapsed().as_millis().try_into().unwrap_or(u64::MAX));
                traffic_stats.rx_bytes = traffic_stats.rx_bytes.saturating_add(rx_bytes);
                traffic_stats.tx_bytes = traffic_stats.tx_bytes.saturating_add(tx_bytes);
                traffic_stats.latest_latency_ms = latest_latency_ms;
                traffic_stats
            }
        }
    }

    fn set_disconnected(&mut self) {
        *self = Self::Disconnected;
    }

    fn set_connecting(&mut self, new_args: &TunnelArgs, disconnect_reason: Option<QuicWgReceiveError>) {
        match self {
            this @ Self::Connected { .. } | this @ Self::Disconnected => {
                *this = Self::Connecting {
                    args: new_args.clone(),
                    connect_error: None,
                    disconnect_reason: disconnect_reason.map(Into::into),
                    offset_traffic_stats: this.traffic_stats(),
                }
            }
            Self::Connecting { args, .. } => *args = new_args.clone(),
        }
    }

    fn set_connected(&mut self, args: &TunnelArgs, conn: Arc<QuicWgConn>, network_config: NetworkConfig, relay: OneRelay, exit: OneExit) {
        *self = Self::Connected { args: args.clone(), conn, network_config, relay, exit, offset_traffic_stats: self.traffic_stats() };
    }

    fn set_connect_error(&mut self, error: TunnelConnectError) {
        let Self::Connecting { connect_error, .. } = self else {
            tracing::error!(
                message_id = "jZGhFRZh",
                "trying to set connect error on non-connecting tunnel state, this should be impossible"
            );
            return;
        };
        *connect_error = Some(error.into())
    }

    pub fn get_conn(&self) -> Option<Arc<QuicWgConn>> {
        match self {
            TunnelState::Disconnected => None,
            TunnelState::Connecting { .. } => None,
            TunnelState::Connected { conn, .. } => Some(conn.clone()),
        }
    }

    fn is_target_state(&self, target_args: &Option<TunnelArgs>) -> bool {
        let current_args = match self {
            Self::Disconnected => None,
            Self::Connecting { args, .. } | Self::Connected { args, .. } => Some(args),
        };
        !self.is_connecting() && current_args == target_args.as_ref()
    }

    async fn maintain(
        mut target_args_recv: Receiver<Option<TunnelArgs>>,
        tunnel_state: Sender<TunnelState>,
        client_state: Arc<ClientState>,
        receive_cb: extern "C" fn(FfiBytes),
    ) -> ! {
        let mut disconnect_reason = None;

        loop {
            let target_args = target_args_recv.borrow_and_update().clone();
            tracing::info!(message_id = "Azzlo6j2", ?target_args, "new target args");

            if !tunnel_state.borrow().is_target_state(&target_args) {
                tracing::info!(message_id = "KT91bgvI", "not in target state");

                // Drop tunnel if args changed and change to connecting or disconnected as desired
                tunnel_state.send_modify(|tunnel_state| match &target_args {
                    None => tunnel_state.set_disconnected(),
                    Some(target_args) => tunnel_state.set_connecting(target_args, disconnect_reason.take()),
                });

                // Try to connect if desired
                if let Some(target_args) = &target_args {
                    match poll_until_change(&mut target_args_recv, client_state.connect(target_args.exit.clone())).await {
                        None => {
                            tracing::info!(
                                message_id = "SmLhzVwY",
                                "target state or tunnel arguments changed while trying to connect"
                            );
                            continue;
                        }
                        Some(Err(error)) => {
                            tracing::info!(message_id = "OfLfwKhf", ?error, "failed to connect");
                            tunnel_state.send_modify(|tunnel_state| tunnel_state.set_connect_error(error));
                            continue;
                        }
                        Some(Ok((conn, network_config, exit, relay))) => {
                            tracing::info!(message_id = "icGquatl", "connected successfully, setting connected state");
                            tunnel_state
                                .send_modify(|tunnel_state| tunnel_state.set_connected(target_args, conn.into(), network_config, relay, exit));
                        }
                    }
                }
            }

            tracing::info!(message_id = "axfILRQy", ?target_args, "reached target state");

            let conn = tunnel_state.borrow().get_conn();
            match conn {
                None => {
                    // reached disconnected target state, do nothing until target state changes
                    poll_until_change(&mut target_args_recv, pending::<()>()).await;
                }
                Some(conn) => {
                    // reached connected target state forward traffic until target state changes or the tunnel fails
                    disconnect_reason = poll_until_change(&mut target_args_recv, async {
                        loop {
                            match conn.receive().await {
                                Ok(packet) => receive_cb(packet.ffi()),
                                Err(error) => {
                                    tracing::error!(message_id = "tls1cZot", ?error, "tunnel failed");
                                    break error;
                                }
                            }
                        }
                    })
                    .await;
                }
            }
        }
    }
}

// Run future, until complete or until the watch channel signals a change.
async fn poll_until_change<T, O>(watch: &mut Receiver<T>, fut: impl Future<Output = O>) -> Option<O> {
    select! {
        _ = watch.changed() => None,
        o = fut => Some(o),
    }
}
