use crate::errors::RelaySelectionError;
use crate::net::{NetworkInterface, new_quic, new_udp};
use crate::quicwg::{QuicWgConnHandshaking, QuicWgConnectError};
use obscuravpn_api::types::OneRelay;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio::time::timeout;

const ABANDON_GRACE_PERIOD: Duration = Duration::from_secs(1);

// Port 53 (DNS) is frequently throttled.
const PORT_53_RTT_PENALTY: Duration = Duration::from_millis(5);

pub struct RelayCandidate {
    pub handshaking: QuicWgConnHandshaking,
    pub port: u16,
    pub relay: OneRelay,
    pub rtt: Duration,
}

impl RelayCandidate {
    pub fn is_better_than(&self, other: &Self) -> bool {
        self.weighted_rtt() < other.weighted_rtt()
    }

    fn weighted_rtt(&self) -> Duration {
        match self.port {
            53 => self.rtt.saturating_add(PORT_53_RTT_PENALTY),
            _ => self.rtt,
        }
    }
}

pub struct RelayHandshakeRace {
    tasks: JoinSet<(Result<(QuicWgConnHandshaking, Duration), QuicWgConnectError>, OneRelay, u16)>,
}

impl RelayHandshakeRace {
    pub async fn abandon(mut self) {
        self.tasks.abort_all();
        let mut abandons = JoinSet::new();
        let mut aborted = 0usize;
        let mut failed = 0usize;
        while let Some(task_result) = self.tasks.join_next().await {
            match task_result {
                Ok((Ok((handshaking, _rtt)), _relay, _port)) => {
                    abandons.spawn(handshaking.abandon());
                }
                Ok((Err(_), _relay, _port)) => failed += 1,
                Err(join_error) => {
                    if join_error.is_cancelled() {
                        aborted += 1;
                    }
                }
            }
        }
        tracing::info!(
            message_id = "Uw4kRz2n",
            aborted,
            failed,
            stopping = abandons.len(),
            "abandoned relay handshake race"
        );
        let _ = timeout(ABANDON_GRACE_PERIOD, async move { while abandons.join_next().await.is_some() {} }).await;
    }

    pub async fn next(&mut self) -> Option<RelayCandidate> {
        while let Some(task_result) = self.tasks.join_next().await {
            let Ok((result, relay, port)) = task_result else { continue };
            match result {
                Ok((handshaking, rtt)) => {
                    tracing::info!(
                        message_id = "7NCuscqm",
                        relay.id,
                        relay.port = port,
                        relay.rtt_ms = rtt.as_millis(),
                        "successfully started handshake with relay"
                    );
                    return Some(RelayCandidate { handshaking, port, relay, rtt });
                }
                Err(error) => {
                    tracing::warn!(
                        message_id = "Drl0nTSh",
                        ?error,
                        relay.id,
                        relay.port = port,
                        "failed to connect during relay selection"
                    );
                }
            }
        }
        None
    }
}

pub fn race_relay_handshakes(
    network_interface: Option<&NetworkInterface>,
    relays: &[OneRelay],
    sni: String,
    use_tcp_tls: bool,
    quic_frame_padding: bool,
    force_small_mtu: bool,
    mtu: Option<u16>,
) -> Result<RelayHandshakeRace, RelaySelectionError> {
    let sni = Arc::new(sni);
    let mut tasks = JoinSet::new();
    let udp = new_udp(network_interface).map_err(RelaySelectionError::UdpSetup)?;
    let quic_endpoint = new_quic(udp, mtu, force_small_mtu).map_err(RelaySelectionError::QuicSetup)?;

    // Maximum number of relays and ports per relay to probe. These limits should be high enough that a non-malicious API server won't exceed them.
    // This prevents memory exhaustion issues in case a malicious API server sends a large number of relays or ports.
    const MAX_RELAYS: usize = 100;
    const MAX_PORTS_PER_RELAY: usize = 5;

    for relay in relays.iter().take(MAX_RELAYS) {
        for &port in relay.ports.iter().take(MAX_PORTS_PER_RELAY) {
            let quic_endpoint = quic_endpoint.clone();
            let relay_addr = (relay.ip_v4, port).into();
            let relay_cert = relay.tls_cert.clone().into();
            let relay = relay.clone();
            let sni = sni.clone();
            let network_interface = network_interface.cloned();
            tasks.spawn(async move {
                let result: Result<(QuicWgConnHandshaking, Duration), QuicWgConnectError> = async {
                    let mut handshaking = match use_tcp_tls {
                        true => {
                            QuicWgConnHandshaking::start_tcp_tls(relay.id.clone(), network_interface.as_ref(), relay_addr, relay_cert, &sni).await
                        }
                        false => {
                            QuicWgConnHandshaking::start_quic(relay.id.clone(), &quic_endpoint, relay_addr, relay_cert, &sni, quic_frame_padding)
                                .await
                        }
                    }?;
                    let rtt = handshaking.measure_rtt().await?;
                    Ok((handshaking, rtt))
                }
                .await;
                (result, relay, port)
            });
        }
    }

    Ok(RelayHandshakeRace { tasks })
}
