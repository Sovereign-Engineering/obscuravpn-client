use std::time::Duration;
use std::{future::Future, sync::Arc};

use futures::future::pending;
use obscuravpn_api::types::{OneExit, OneRelay};
use strum::EnumIs;
use tokio::runtime::Runtime;
use tokio::select;
use tokio::sync::watch::{Receiver, Sender, channel};
use tokio::time::{Instant, sleep_until};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::errors::{ErrorAt, TunnelConnectError};
use crate::exit_selection::ExitSelectionState;
use crate::ffi_helpers::{FfiBytes, FfiBytesExt};
use crate::manager::ManagerTrafficStats;
use crate::net::NetworkInterface;
use crate::network_config::{DnsContentBlock, TunnelNetworkConfig};
use crate::quicwg::{QuicWgReceiveError, QuicWgTrafficStats};
use crate::{client_state::ClientState, manager::TunnelArgs, quicwg::QuicWgConn};

#[derive(Clone, Debug)]
pub struct TargetState {
    pub tunnel_args: Option<TunnelArgs>,
    pub network_interface: Option<NetworkInterface>,
    pub dns_content_block: DnsContentBlock,
}

#[derive(derive_more::Debug, EnumIs)]
pub enum TunnelState {
    Disconnected,
    Connecting {
        args: TunnelArgs,
        connect_error: Option<ErrorAt<TunnelConnectError>>,
        disconnect_reason: Option<ErrorAt<QuicWgReceiveError>>,
        offset_traffic_stats: ManagerTrafficStats,
        network_interface: Option<NetworkInterface>,
    },
    Connected {
        args: TunnelArgs,
        #[debug(skip)]
        conn: Arc<QuicWgConn>,
        network_config: TunnelNetworkConfig,
        relay: OneRelay,
        exit: OneExit,
        offset_traffic_stats: ManagerTrafficStats,
        network_interface: NetworkInterface,
        dns_content_block: DnsContentBlock,
    },
}

impl TunnelState {
    pub fn new(
        runtime: &Runtime,
        client_state: Arc<ClientState>,
        receive_cb: extern "C" fn(FfiBytes),
        cancel: CancellationToken,
    ) -> (Sender<TargetState>, Receiver<TunnelState>) {
        let dns_content_block = client_state.get_config().dns_content_block;
        let (target_state_send, target_state_recv) = channel(TargetState { tunnel_args: None, network_interface: None, dns_content_block });
        let (tunnel_state_send, tunnel_state_recv) = channel(TunnelState::Disconnected);
        runtime.spawn(async move {
            cancel
                .run_until_cancelled(Self::maintain(target_state_recv, tunnel_state_send, client_state, receive_cb))
                .await;
        });
        (target_state_send, tunnel_state_recv)
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

    fn set_connecting(&mut self, new_args: &TunnelArgs, network_interface: &Option<NetworkInterface>, disconnect_reason: Option<QuicWgReceiveError>) {
        match self {
            this @ Self::Connected { .. } | this @ Self::Disconnected => {
                *this = Self::Connecting {
                    args: new_args.clone(),
                    connect_error: None,
                    disconnect_reason: disconnect_reason.map(Into::into),
                    offset_traffic_stats: this.traffic_stats(),
                    network_interface: network_interface.clone(),
                }
            }
            Self::Connecting { args, .. } => *args = new_args.clone(),
        }
    }

    fn set_connected(
        &mut self,
        args: &TunnelArgs,
        network_interface: &NetworkInterface,
        conn: Arc<QuicWgConn>,
        network_config: TunnelNetworkConfig,
        relay: OneRelay,
        exit: OneExit,
        dns_content_block: DnsContentBlock,
    ) {
        *self = Self::Connected {
            args: args.clone(),
            network_interface: network_interface.clone(),
            conn,
            network_config,
            relay,
            exit,
            offset_traffic_stats: self.traffic_stats(),
            dns_content_block,
        };
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

    fn is_target(&self, target_state: &TargetState) -> bool {
        let TargetState { tunnel_args: target_tunnel_args, network_interface: target_network, dns_content_block } = target_state;
        let (current_args, current_network_interface, current_dns_content_block) = match self {
            Self::Disconnected => (None, None, None),
            Self::Connecting { .. } => return false,
            Self::Connected { args, network_interface, dns_content_block, .. } => (Some(args), Some(network_interface), Some(dns_content_block)),
        };
        current_args == target_tunnel_args.as_ref()
            && current_network_interface == target_network.as_ref()
            && current_dns_content_block.is_none_or(|current| current == dns_content_block)
    }

    async fn maintain(
        mut target_state_recv: Receiver<TargetState>,
        tunnel_state: Sender<TunnelState>,
        client_state: Arc<ClientState>,
        receive_cb: extern "C" fn(FfiBytes),
    ) -> ! {
        // Delay processing new states or retrying after error for at least this long.
        const DEBOUNCE_PERIOD: Duration = Duration::from_secs(1);

        let mut last_start: Option<Instant> = None;
        let mut disconnect_reason = None;
        let mut selection_state = ExitSelectionState::default();

        loop {
            if let Some(last_start) = last_start {
                sleep_until(last_start + DEBOUNCE_PERIOD).await;
            }
            last_start = Some(Instant::now());

            let target_state = target_state_recv.borrow_and_update().clone();
            tracing::info!(message_id = "Azzlo6j2", ?target_state, "new target state");

            if !tunnel_state.borrow().is_target(&target_state) || disconnect_reason.is_some() {
                tracing::info!(message_id = "KT91bgvI", ?disconnect_reason, "not in target state or tunnel broke");

                if target_state.tunnel_args.is_none() {
                    // Target state changed to disconnected, which means we will disconnect, but are in another state.
                    // This is the right time for key rotations without unnecessarily rotating keys of permanently unused devices.
                    if let Err(error) = client_state.rotate_wireguard_key_if_required() {
                        tracing::info!(message_id = "MsHuAlwT", ?error, "key rotation failed");
                    }
                }

                // Drop tunnel if args changed and change to connecting or disconnected as desired
                tunnel_state.send_modify(|tunnel_state| match &target_state {
                    TargetState { tunnel_args: None, network_interface: _, dns_content_block: _ } => tunnel_state.set_disconnected(),
                    TargetState { tunnel_args: Some(target_args), network_interface, dns_content_block: _ } => {
                        tunnel_state.set_connecting(target_args, network_interface, disconnect_reason.take())
                    }
                });

                // Connect if desired and possible. If target state is reached maintain it until the target changes.
                match &target_state {
                    TargetState { tunnel_args: Some(target_args), network_interface: Some(target_network_interface), dns_content_block } => {
                        match poll_until_change(
                            &mut target_state_recv,
                            client_state.connect(&target_args.exit, Some(target_network_interface), &mut selection_state),
                        )
                        .await
                        {
                            None => {
                                tracing::info!(
                                    message_id = "SmLhzVwY",
                                    "target state or tunnel arguments changed while trying to connect"
                                );
                            }
                            Some(Err(error)) => {
                                tracing::info!(message_id = "OfLfwKhf", ?error, "failed to connect");
                                tunnel_state.send_modify(|tunnel_state| tunnel_state.set_connect_error(error));
                            }
                            Some(Ok((conn, mut network_config, exit, relay))) => {
                                tracing::info!(message_id = "icGquatl", "connected successfully, setting connected state");
                                selection_state = ExitSelectionState::default();
                                network_config.apply_dns_content_block(&exit.provider_name, *dns_content_block);
                                let conn = Arc::new(conn);
                                tunnel_state.send_modify(|tunnel_state| {
                                    tunnel_state.set_connected(
                                        target_args,
                                        target_network_interface,
                                        conn.clone(),
                                        network_config,
                                        relay,
                                        exit,
                                        *dns_content_block,
                                    )
                                });
                                // reached connected target state forward traffic until target state changes or the tunnel fails
                                disconnect_reason = poll_until_change(&mut target_state_recv, async {
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
                    TargetState { tunnel_args: None, network_interface: _, dns_content_block: _ } => {
                        tracing::info!(message_id = "axfILRQy", "reached disconnected target state");
                        selection_state = ExitSelectionState::default();
                        // nothing to do until target args change
                        poll_until_change(&mut target_state_recv, pending::<()>()).await;
                    }
                    TargetState { tunnel_args: Some(_), network_interface: None, dns_content_block: _ } => {
                        tracing::warn!(message_id = "0K9Nep8g", "stuck in connecting state without target interface");
                        selection_state = ExitSelectionState::default();
                        tunnel_state.send_modify(|tunnel_state| tunnel_state.set_connect_error(TunnelConnectError::NoInternet));
                        // nothing to do until target args changes or a network device becomes available
                        poll_until_change(&mut target_state_recv, pending::<()>()).await;
                    }
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
