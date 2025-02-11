use crate::errors::RelaySelectionError;
use crate::net::{new_quic, new_udp};
use crate::quicwg::{QuicWgConnHandshaking, QuicWgConnectError};
use flume::{bounded, Receiver, SendError};
use obscuravpn_api::types::OneRelay;
use std::time::Duration;
use tokio::spawn;
use tokio::task::JoinSet;

pub fn race_relay_handshakes(relays: Vec<OneRelay>) -> Result<Receiver<(OneRelay, u16, Duration, QuicWgConnHandshaking)>, RelaySelectionError> {
    let mut tasks = JoinSet::new();
    let udp = new_udp(None).map_err(RelaySelectionError::UdpSetup)?;
    let quic_endpoint = new_quic(udp).map_err(RelaySelectionError::QuicSetup)?;

    for relay in relays {
        for &port in &relay.ports {
            let quic_endpoint = quic_endpoint.clone();
            let relay_addr = (relay.ip_v4, port).into();
            let relay_cert = relay.tls_cert.clone().into();
            let relay = relay.clone();
            tasks.spawn(async move {
                let result: Result<(QuicWgConnHandshaking, Duration), QuicWgConnectError> = async {
                    let mut handshaking = QuicWgConnHandshaking::start(relay.id.clone(), &quic_endpoint, relay_addr, relay_cert).await?;
                    let rtt = handshaking.measure_rtt().await?;
                    Ok((handshaking, rtt))
                }
                .await;
                (result, relay, port)
            });
        }
    }

    let (sender, receiver) = bounded(0);
    spawn(async move {
        while let Some(Ok((result, relay, port))) = tasks.join_next().await {
            let (handshaking, rtt) = match result {
                Ok(ok) => ok,
                Err(error) => {
                    tracing::warn!(?error, relay.id, port, "failed to connect during relay selection");
                    continue;
                }
            };
            tracing::info!(relay.id, port, rtt_ms = rtt.as_millis(), "successfully started handshake with relay");
            if let Err(SendError((_, _, _, handshaking))) = sender.send_async((relay, port, rtt, handshaking)).await {
                spawn(handshaking.abandon());
            }
        }
    });
    Ok(receiver)
}
