use boringtun::noise::{Tunn, TunnResult};
use boringtun::x25519::{PublicKey, StaticSecret};
use bytes::Bytes;
use futures::Stream;
use futures::StreamExt;
use futures::stream::unfold;
use obscuravpn_api::relay_protocol::{MessageCode, MessageContext, MessageHeader, PROTOCOL_IDENTIFIER, RelayOpCode, RelayResponseCode};
use quinn::crypto::rustls::QuicClientConfig;
use quinn::rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use quinn::rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use quinn::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use quinn::rustls::{CertificateError, DigitallySignedStruct, SignatureScheme};
use quinn::{ClientConfig, MtuDiscoveryConfig, rustls};
use rand::random;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::mem;
use std::net::{Ipv4Addr, SocketAddr};
use std::num::{NonZeroU32, Saturating};
use std::ops::ControlFlow;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use strum::Display;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf};
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::time::{Duration, Instant};
use tokio::time::{sleep_until, timeout};
use tokio::{io, select, spawn};
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;
use uuid::Uuid;

use crate::liveness::LivenessChecker;
use crate::tokio::AbortOnDrop;

const WG_FIRST_HANDSHAKE_RESENDS: usize = 25; // 2.5s per handshake.
const WG_FIRST_HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(100);

/// Ideally we would have a shorter QUIC idle timeout at the beginning and no timeout once the connection starts but this is not supported by quinn.
pub const QUIC_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

const QUIC_STEP_TIMEOUT: Duration = Duration::from_secs(30);

/// How fast to call `update_timers`.
///
/// In the boringtun repo they call it at 4Hz, however we have traditionally called it at 1Hz and doesn't seem to have any problems.
const WG_TIMER_TICK: Duration = Duration::from_secs(1);
pub const TUNNEL_MTU: u16 = 1280;
const LIVENESS_MTU: u16 = 100;

#[derive(Debug, Error)]
pub enum QuicWgReceiveError {
    #[error("tunnel is dead")]
    TunnelDead,
    #[error("quic receive error: {0}")]
    QuicReceiveError(io::Error),
}

#[derive(Debug, Error)]
pub enum QuicWgConnectError {
    #[error("crypto config: {0}")]
    CryptoConfig(anyhow::Error),
    #[error("quic config: {0}")]
    QuicConfig(quinn::ConnectError),
    #[error("quic connect: {0}")]
    TransportConnect(io::Error),
    #[error("relay handshake: {0}")]
    RelayHandshake(#[from] QuicWgRelayHandshakeError),
    #[error("wireguard handshake: {0}")]
    WireguardHandshake(QuicWgWireguardHandshakeError),
}

#[derive(Debug, Error)]
pub enum QuicWgRelayHandshakeError {
    #[error("could not open control stream: {0}")]
    ControlStreamInitError(quinn::ConnectionError),
    #[error("could not receive message from control stream: {0}")]
    ControlStreamMessageReceiveError(io::Error),
    #[error("could not read protocol identifier from control stream: {0}")]
    ProtocolIdentifierReceiveFailed(io::Error),
    #[error("timeout {0}")]
    Timeout(&'static str),
    #[error("relay sent unexpected protocol indentifier: {0:#034x}")]
    UnexpectedProtocolIdentifierReceived(u128),
    #[error("could not write to control stream: {0}")]
    ControlStreamWriteError(io::Error),
    #[error("received {0}")]
    ReceivedErrorResponse(RelayErrorResponse),
}

#[derive(Debug, Error)]
#[error("relay error response code {error_code}: {message}")]
pub struct RelayErrorResponse {
    error_code: NonZeroU32,
    message: String,
}

#[derive(Debug, Error)]
#[error("unexpected relay op code {0:?}")]
pub struct UnexpectedOpCode(RelayOpCode);

impl RelayErrorResponse {
    pub fn new(error_code: NonZeroU32, message: &[u8]) -> Self {
        Self { error_code, message: String::from_utf8_lossy(message).into() }
    }
}

#[derive(Debug, Error)]
pub enum QuicWgWireguardHandshakeError {
    #[error("could not construct inititialization message")]
    InitMessageConstructError,
    #[error("could not send inititialization message: {0}")]
    InitMessageSendError(io::Error),
    #[error("could not receive response message: {0}")]
    RespMessageReceiveError(io::Error),
    #[error("response timeout")]
    RespMessageTimeout,
}

pub struct QuicWgConn {
    wg_state: Mutex<WgState>,
    wg_sender: WgSender,
    wg_receiver: WgReceiver,
    client_public_key: PublicKey,
    exit_public_key: PublicKey,
    _tcp_tls_sender_abort: Option<AbortOnDrop>,
    _quic_control_stream: Option<(quinn::SendStream, quinn::RecvStream)>,
}

#[derive(Clone, Copy, Debug)]
pub struct QuicWgTrafficStats {
    pub connected_at: Instant,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub latest_latency_ms: u16,
}

struct WgState {
    buffer: Vec<u8>,
    next_wg_timers_tick: Instant,
    next_liveness_poll: Instant,
    tick_stats: TickStats,
    traffic_stats: QuicWgTrafficStats,
    wg: Tunn,
    liveness_checker: LivenessChecker,
}

#[derive(Clone, Copy, Debug, Default)]
struct TickStats {
    ip_tx_count: Saturating<u64>,
    wg_tx_count: Saturating<u64>,
    ip_rx_count: Saturating<u64>,
    wg_rx_count: Saturating<u64>,
    min_ip_tx_size: Option<usize>,
    max_ip_tx_size: Option<usize>,
    min_ip_rx_size: Option<usize>,
    max_ip_rx_size: Option<usize>,
}

#[derive(Debug, Display, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TransportKind {
    Quic,
    TcpTls,
}

impl QuicWgConn {
    pub async fn connect(
        relay_handshaking: QuicWgConnHandshaking,
        client_secret_key: StaticSecret,
        exit_public_key: PublicKey,
        client_ip_v4: Ipv4Addr,
        ping_target_ip_v4: Ipv4Addr,
        token: Uuid,
    ) -> Result<Self, QuicWgConnectError> {
        let client_public_key = PublicKey::from(&client_secret_key);
        let (mut wg_sender, mut wg_receiver, quic_control_stream, tcp_tls_sender_abort) =
            relay_handshaking.authenticate(token).await?.into_wg_send_recv();
        tracing::info!(message_id = "UROUZerU", "completed handshake with relay");

        let index = random();
        let mut wg = Tunn::new(client_secret_key, exit_public_key, None, None, index, None).unwrap();
        Self::first_wg_handshake(&mut wg, &mut wg_sender, &mut wg_receiver, WG_FIRST_HANDSHAKE_RESENDS)
            .await
            .map_err(QuicWgConnectError::WireguardHandshake)?;
        tracing::info!(message_id = "TJ4nH30h", "connected to exit");
        let now = Instant::now();
        let wg_state = Mutex::new(WgState {
            wg,
            traffic_stats: QuicWgTrafficStats { connected_at: now, tx_bytes: 0, rx_bytes: 0, latest_latency_ms: 0 },
            buffer: vec![0u8; u16::MAX as usize],
            next_wg_timers_tick: now + WG_TIMER_TICK,
            next_liveness_poll: now,
            liveness_checker: LivenessChecker::new(LIVENESS_MTU, client_ip_v4, ping_target_ip_v4),
            tick_stats: Default::default(),
        });
        Ok(Self {
            wg_receiver,
            wg_sender,
            wg_state,
            client_public_key,
            exit_public_key,
            _tcp_tls_sender_abort: tcp_tls_sender_abort,
            _quic_control_stream: quic_control_stream,
        })
    }

    fn build_first_wg_handshake_init(wg: &mut Tunn) -> Result<Vec<u8>, QuicWgWireguardHandshakeError> {
        let mut buf = vec![0u8; u16::MAX as usize];
        let data = match wg.format_handshake_initiation(&mut buf, true) {
            TunnResult::WriteToNetwork(data) => data.to_vec(),
            _ => return Err(QuicWgWireguardHandshakeError::InitMessageConstructError),
        };
        Ok(data)
    }

    async fn wait_for_first_handshake_response(
        wg: &mut Tunn,
        wg_receiver: &mut WgReceiver,
        wg_sender: &WgSender,
    ) -> Result<(), QuicWgWireguardHandshakeError> {
        let mut buf = vec![0u8; u16::MAX as usize];
        timeout(WG_FIRST_HANDSHAKE_TIMEOUT, async {
            while wg.time_since_last_handshake().is_none() {
                let mut datagram = wg_receiver
                    .receive_wg_message()
                    .await
                    .map_err(QuicWgWireguardHandshakeError::RespMessageReceiveError)?;
                loop {
                    let res = wg.decapsulate(None, &datagram, &mut buf);
                    match Self::handle_result(wg_sender, res) {
                        ControlFlow::Continue(()) => {
                            datagram.truncate(0);
                            continue;
                        }
                        ControlFlow::Break(Some(_)) => tracing::warn!(message_id = "d8pzbt5Z", "unexpected packet during first WG handshake"),
                        ControlFlow::Break(None) => break,
                    }
                }
            }
            Ok(())
        })
        .await
        .map_err(|_| QuicWgWireguardHandshakeError::RespMessageTimeout)?
    }

    async fn first_wg_handshake(
        wg: &mut Tunn,
        wg_sender: &mut WgSender,
        wg_receiver: &mut WgReceiver,
        resends: usize,
    ) -> Result<(), QuicWgWireguardHandshakeError> {
        let handshake_init = Self::build_first_wg_handshake_init(wg)?;
        let mut resends = resends;
        loop {
            resends -= 1;
            wg_sender.send_wg_message(&handshake_init);
            match Self::wait_for_first_handshake_response(wg, wg_receiver, wg_sender).await {
                Ok(()) => return Ok(()),
                Err(err) => match err {
                    QuicWgWireguardHandshakeError::RespMessageTimeout => {
                        tracing::info!(message_id = "dHFvpzvl", "exit handshake timeout, packet may have gotten lost");
                        if resends == 0 {
                            tracing::info!(
                                message_id = "ZvQp8VQQ",
                                "too many exit handshake resend attempts, exit may not be configured"
                            );
                            break;
                        }
                    }
                    err => return Err(err),
                },
            }
        }
        Err(QuicWgWireguardHandshakeError::RespMessageTimeout)
    }

    fn handle_result(wg_sender: &WgSender, res: TunnResult<'_>) -> ControlFlow<Option<Vec<u8>>> {
        match res {
            TunnResult::Done => ControlFlow::Break(None),
            TunnResult::WriteToNetwork(wg_message) => {
                wg_sender.send_wg_message(wg_message);
                ControlFlow::Continue(())
            }
            TunnResult::WriteToTunnelV4(packet, ..) | TunnResult::WriteToTunnelV6(packet, ..) => ControlFlow::Break(Some(packet.to_vec())),
            TunnResult::Err(error) => {
                tracing::warn!(message_id = "uQ0xQcPP", ?error, "wireguard error");
                ControlFlow::Break(None)
            }
        }
    }

    pub fn send(&self, packets: &[&[u8]]) {
        let mut wg_state = self.wg_state.lock().unwrap();
        if let Some(packet) = wg_state.liveness_checker.sent_traffic() {
            self.send_single_packet(&mut wg_state, &packet);
        }
        for packet in packets {
            wg_state.traffic_stats.tx_bytes += packet.len() as u64;
            wg_state.tick_stats.ip_tx_count += 1;
            wg_state.tick_stats.min_ip_tx_size = Some(wg_state.tick_stats.min_ip_tx_size.unwrap_or(usize::MAX).min(packet.len()));
            wg_state.tick_stats.max_ip_tx_size = Some(wg_state.tick_stats.max_ip_tx_size.unwrap_or(0).max(packet.len()));
            self.send_single_packet(&mut wg_state, packet);
        }
    }

    pub fn wake(&self) {
        let mut wg_state = self.wg_state.lock().unwrap();
        let packet = wg_state.liveness_checker.wake();
        self.send_single_packet(&mut wg_state, &packet);
    }

    fn send_single_packet(&self, wg_state: &mut WgState, packet: &[u8]) {
        match wg_state.wg.encapsulate(packet, &mut wg_state.buffer) {
            TunnResult::Done => tracing::error!(message_id = "10g8g1D1", "WG encapsulate did not yield a datagram to send"),
            TunnResult::Err(error) => tracing::warn!(message_id = "MAvGA9tf", ?error, "wireguard error"),
            TunnResult::WriteToNetwork(wg_message) => {
                wg_state.tick_stats.wg_tx_count += 1;
                self.wg_sender.send_wg_message(wg_message);
            }
            TunnResult::WriteToTunnelV4(_, _) | TunnResult::WriteToTunnelV6(_, _) => {
                tracing::error!(message_id = "mOwsH8Eu", "WG encapsulate yielded a received ip packet")
            }
        }
    }

    pub async fn receive(&self) -> Result<Vec<u8>, QuicWgReceiveError> {
        loop {
            let next_liveness_poll;
            let next_wg_timers_tick;
            {
                let wg_state = &mut *self.wg_state.lock().unwrap();
                next_liveness_poll = wg_state.next_liveness_poll;
                next_wg_timers_tick = wg_state.next_wg_timers_tick;
            }

            select! {
                biased;
                _ = sleep_until(next_wg_timers_tick) => {
                    let wg_state = &mut *self.wg_state.lock().unwrap();
                    tracing::info!(
                        message_id = "WKqFjXMA",
                        tick_stats =? wg_state.tick_stats,
                    );
                    wg_state.tick_stats = TickStats::default();
                    loop {
                        let timer_result = wg_state.wg.update_timers(&mut wg_state.buffer);
                        match Self::handle_result(&self.wg_sender, timer_result) {
                            ControlFlow::Continue(()) => continue,
                            ControlFlow::Break(Some(_)) => tracing::warn!(message_id = "nmuKdNnr", "unexpected packet during update_timers"),
                            ControlFlow::Break(None) => break,
                        }
                    }
                    wg_state.next_wg_timers_tick = Instant::now() + WG_TIMER_TICK;
                }
                result = self.wg_receiver.receive_wg_message() => {
                    let mut wg_message = result.map_err(QuicWgReceiveError::QuicReceiveError)?;
                    let mut wg_state = self.wg_state.lock().unwrap();
                    let WgState {buffer, wg, traffic_stats, tick_stats, liveness_checker, .. } = &mut *wg_state;
                    tick_stats.wg_rx_count += 1;
                    loop {
                        let res = wg.decapsulate(None, &wg_message, buffer);
                        if let TunnResult::WriteToNetwork(..) = &res {
                            tick_stats.wg_tx_count += 1;
                        }
                        match Self::handle_result(&self.wg_sender, res) {
                            ControlFlow::Continue(()) => {
                                wg_message.truncate(0);
                                continue
                            }
                            ControlFlow::Break(Some(packet)) => {
                                tick_stats.ip_rx_count += 1;
                                tick_stats.min_ip_rx_size = Some(tick_stats.min_ip_rx_size.unwrap_or(usize::MAX).min(packet.len()));
                                tick_stats.max_ip_rx_size = Some(tick_stats.max_ip_rx_size.unwrap_or(0).max(packet.len()));
                                traffic_stats.rx_bytes += packet.len() as u64;
                                if let Some(latest_latency) = liveness_checker.process_potential_probe_response(&packet) {
                                    traffic_stats.latest_latency_ms = u16::try_from(latest_latency.as_millis()).unwrap_or(u16::MAX);
                                    break
                                }
                                return Ok(packet)
                            },
                            ControlFlow::Break(None) => break,
                        }
                    }
                }
                _ = sleep_until(next_liveness_poll) => {
                    let wg_state = &mut*self.wg_state.lock().unwrap();
                    wg_state.next_liveness_poll = loop {
                        match wg_state.liveness_checker.poll() {
                            crate::liveness::LivenessCheckerPoll::Dead => break Err(QuicWgReceiveError::TunnelDead),
                            crate::liveness::LivenessCheckerPoll::AliveUntil(pending_until) => break Ok(pending_until),
                            crate::liveness::LivenessCheckerPoll::SendPacket(packet) => self.send_single_packet(wg_state, &packet),
                        }
                    }?.into();
                }
            }
        }
    }

    pub fn traffic_stats(&self) -> QuicWgTrafficStats {
        self.wg_state.lock().unwrap().traffic_stats
    }

    pub fn exit_public_key(&self) -> PublicKey {
        self.exit_public_key
    }

    pub fn client_public_key(&self) -> PublicKey {
        self.client_public_key
    }

    pub fn transport(&self) -> TransportKind {
        match self.wg_sender {
            WgSender::Quic { .. } => TransportKind::Quic,
            WgSender::TcpTls { .. } => TransportKind::TcpTls,
        }
    }
}

pub struct QuicWgConnHandshaking {
    relay_id: String,
    port: u16,
    transport: Transport,
}

impl QuicWgConnHandshaking {
    pub async fn start_quic(
        relay_id: String,
        quic_endpoint: &quinn::Endpoint,
        relay_addr: SocketAddr,
        relay_cert: CertificateDer<'static>,
        relay_sni: &str,
        pad_to_mtu: bool,
    ) -> Result<Self, QuicWgConnectError> {
        let port = relay_addr.port();
        tracing::info!(
            message_id = "AYsfThUG",
            "starting quic wg relay handshake with {} port {}",
            &relay_id,
            port
        );
        let quic_config = Self::quic_config(relay_cert, pad_to_mtu, TUNNEL_MTU).map_err(QuicWgConnectError::CryptoConfig)?;
        let connecting = quic_endpoint
            .connect_with(quic_config.clone(), relay_addr, relay_sni)
            .map_err(QuicWgConnectError::QuicConfig)?;
        let connection = connecting.await.map_err(io::Error::other).map_err(QuicWgConnectError::TransportConnect)?;
        let (send, recv) = connection.open_bi().await.map_err(QuicWgRelayHandshakeError::ControlStreamInitError)?;
        let mut this = Self { relay_id, port, transport: Transport::Quic { conn: connection, send, recv } };
        this.exchange_protocol_identifiers().await?;
        Ok(this)
    }

    pub async fn start_tcp_tls(
        relay_id: String,
        relay_addr: SocketAddr,
        relay_cert: CertificateDer<'static>,
        relay_sni: &str,
    ) -> Result<Self, QuicWgConnectError> {
        let tcp_stream = TcpStream::connect(relay_addr).await.map_err(QuicWgConnectError::TransportConnect)?;
        if let Err(error) = tcp_stream.set_nodelay(true) {
            tracing::warn!(message_id = "k9KRCm3G", ?error, "failed to set tcp nodelay");
        }
        let tls_connector = Self::tcp_tls_config(relay_cert).map_err(QuicWgConnectError::CryptoConfig)?;
        let server_name = relay_sni
            .to_string()
            .try_into()
            .map_err(Into::into)
            .map_err(QuicWgConnectError::CryptoConfig)?;
        let tls_stream = tls_connector
            .connect(server_name, tcp_stream)
            .await
            .map_err(QuicWgConnectError::TransportConnect)?;
        let mut this = Self { relay_id, port: relay_addr.port(), transport: Transport::TcpTls(tls_stream) };
        this.exchange_protocol_identifiers().await?;
        Ok(this)
    }

    async fn exchange_protocol_identifiers(&mut self) -> Result<(), QuicWgRelayHandshakeError> {
        let mut buffer = PROTOCOL_IDENTIFIER.to_be_bytes();
        match &mut self.transport {
            Transport::Quic { send, recv, .. } => {
                send.write_all(&buffer)
                    .await
                    .map_err(io::Error::other)
                    .map_err(QuicWgRelayHandshakeError::ControlStreamWriteError)?;
                recv.read_exact(&mut buffer)
                    .await
                    .map_err(io::Error::other)
                    .map_err(QuicWgRelayHandshakeError::ProtocolIdentifierReceiveFailed)?;
            }
            Transport::TcpTls(tls_stream) => {
                tls_stream
                    .write_all(&buffer)
                    .await
                    .map_err(QuicWgRelayHandshakeError::ControlStreamWriteError)?;
                tls_stream.flush().await.map_err(QuicWgRelayHandshakeError::ControlStreamWriteError)?;
                tls_stream
                    .read_exact(&mut buffer)
                    .await
                    .map_err(QuicWgRelayHandshakeError::ProtocolIdentifierReceiveFailed)?;
            }
        }
        let relay_protocol_identifier = u128::from_be_bytes(buffer);
        if relay_protocol_identifier != PROTOCOL_IDENTIFIER {
            return Err(QuicWgRelayHandshakeError::UnexpectedProtocolIdentifierReceived(relay_protocol_identifier));
        }
        Ok(())
    }

    pub async fn measure_rtt(&mut self) -> Result<Duration, QuicWgConnectError> {
        let mut start_time = Instant::now();
        let mut min_rtt = Duration::MAX;
        for _ in 0..3 {
            self.send_op(RelayOpCode::Ping, &[]).await?;
            self.recv_ok_resp().await?;
            let end_time = Instant::now();
            if let Some(last_rtt) = end_time.checked_duration_since(start_time) {
                min_rtt = min_rtt.min(last_rtt);
            }
            start_time = end_time;
        }
        tracing::info!(
            message_id = "CyF9avyp",
            "relay {} port {} min rtt is {}ms",
            &self.relay_id,
            self.port,
            min_rtt.as_millis()
        );
        Ok(min_rtt)
    }

    async fn authenticate(mut self, token: Uuid) -> Result<Transport, QuicWgConnectError> {
        self.send_op(RelayOpCode::Token, token.as_bytes()).await?;
        self.recv_ok_resp().await?;
        tracing::info!(message_id = "3rOUXFti", "relay confirmed token");
        let Self { transport, .. } = self;
        Ok(transport)
    }

    async fn stop(&mut self) -> Result<(), QuicWgRelayHandshakeError> {
        tracing::info!(message_id = "eTR2QPCB", "sending stop op to relay {} port {}", &self.relay_id, &self.port,);
        self.send_op(RelayOpCode::Stop, &[]).await?;
        self.recv_ok_resp().await?;
        tracing::info!(message_id = "3BwlgMb7", "relay {} port {} confirmed stop", &self.relay_id, self.port);
        Ok(())
    }

    pub async fn abandon(mut self) {
        if let Err(error) = self.stop().await {
            tracing::warn!(message_id = "b0UeytEt", ?error, "error while abandoning handshake")
        } else {
            match &mut self.transport {
                Transport::Quic { send, .. } => {
                    _ = send.finish();
                    _ = send.stopped().await;
                }
                Transport::TcpTls(tls_stream) => {
                    _ = tls_stream.shutdown().await;
                }
            }
            drop(self);
        }
    }

    fn rustls_config(relay_cert: CertificateDer<'static>) -> Result<rustls::ClientConfig, anyhow::Error> {
        let default_provider = Arc::new(rustls::crypto::ring::default_provider());
        let crypto = rustls::ClientConfig::builder_with_provider(default_provider.clone())
            .with_safe_default_protocol_versions()?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(VerifyVpnServerCert { cert: relay_cert, provider: default_provider }))
            .with_no_client_auth();
        Ok(crypto)
    }

    fn tcp_tls_config(relay_cert: CertificateDer<'static>) -> Result<TlsConnector, anyhow::Error> {
        let mut crypto = Self::rustls_config(relay_cert)?;
        crypto.alpn_protocols = vec![b"h2".to_vec()];
        Ok(TlsConnector::from(Arc::new(crypto)))
    }

    fn quic_config(relay_cert: CertificateDer<'static>, pad_to_mtu: bool, tunnel_mtu: u16) -> Result<ClientConfig, anyhow::Error> {
        let mut crypto = Self::rustls_config(relay_cert)?;
        crypto.alpn_protocols = vec![b"h3".to_vec()];
        let crypto = QuicClientConfig::try_from(crypto)?;
        let mut client_cfg = ClientConfig::new(Arc::new(crypto));
        let mut transport_config = quinn::TransportConfig::default();
        transport_config.max_concurrent_uni_streams(0u8.into());
        transport_config.max_concurrent_bidi_streams(0u8.into());
        let quic_mtu = tunnel_mtu + 70;
        transport_config.initial_mtu(quic_mtu);
        transport_config.min_mtu(quic_mtu);
        let mut mtu_discovery_config = MtuDiscoveryConfig::default();
        mtu_discovery_config.upper_bound(quic_mtu);
        transport_config.mtu_discovery_config(Some(mtu_discovery_config));
        transport_config.max_idle_timeout(Some(QUIC_IDLE_TIMEOUT.try_into()?));
        transport_config.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
        transport_config.pad_to_mtu(pad_to_mtu);
        client_cfg.transport_config(Arc::new(transport_config));
        Ok(client_cfg)
    }

    async fn recv_ok_resp(&mut self) -> Result<(), QuicWgRelayHandshakeError> {
        let inner = self.recv_ok_resp_no_timeout();
        timeout(QUIC_STEP_TIMEOUT, inner)
            .await
            .map_err(|_| QuicWgRelayHandshakeError::Timeout("awaiting op response"))
            .flatten()
    }

    async fn recv_ok_resp_no_timeout(&mut self) -> Result<(), QuicWgRelayHandshakeError> {
        loop {
            let (message_code, context_id, arg) = match &mut self.transport {
                Transport::Quic { recv, .. } => recv_message(recv).await,
                Transport::TcpTls(tls_stream) => recv_message(tls_stream).await,
            }
            .map_err(QuicWgRelayHandshakeError::ControlStreamMessageReceiveError)?;
            let MessageCode::Response(response_code) = message_code else {
                tracing::warn!(
                    message_id = "xfums1F8",
                    ?message_code,
                    "ignoring unexpected relay initiated message during handshake"
                );
                continue;
            };
            if context_id != MessageContext::MIN_CLIENT_INITIATED {
                tracing::warn!(message_id = "EfCIJy4z", ?context_id, "ignoring response with non-zero context id",);
                continue;
            }
            match response_code {
                RelayResponseCode::Ok => {
                    if !arg.is_empty() {
                        tracing::warn!(message_id = "xd0PY4bH", "ignoring {} additional payload bytes on ok response", arg.len());
                    }
                }
                RelayResponseCode::Error(error_code) => {
                    return Err(QuicWgRelayHandshakeError::ReceivedErrorResponse(RelayErrorResponse::new(
                        error_code, &arg,
                    )));
                }
            }
            break Ok(());
        }
    }

    async fn send_op(&mut self, op: RelayOpCode, arg: &[u8]) -> Result<(), QuicWgRelayHandshakeError> {
        let inner = self.send_op_no_timeout(op, arg);
        timeout(QUIC_STEP_TIMEOUT, inner)
            .await
            .map_err(|_| QuicWgRelayHandshakeError::Timeout("sending op"))
            .flatten()
    }

    async fn send_op_no_timeout(&mut self, op: RelayOpCode, arg: &[u8]) -> Result<(), QuicWgRelayHandshakeError> {
        let op = MessageCode::Op(op);
        let context_id = MessageContext::MIN_CLIENT_INITIATED;
        match &mut self.transport {
            Transport::Quic { send, .. } => send_message(send, op, context_id, arg).await,
            Transport::TcpTls(tls_stream) => send_message(tls_stream, op, context_id, arg).await,
        }
        .map_err(QuicWgRelayHandshakeError::ControlStreamWriteError)
    }

    pub fn transport_kind(self) -> TransportKind {
        match self.transport {
            Transport::Quic { .. } => TransportKind::Quic,
            Transport::TcpTls(..) => TransportKind::TcpTls,
        }
    }
}

enum Transport {
    Quic {
        conn: quinn::Connection,
        send: quinn::SendStream,
        recv: quinn::RecvStream,
    },
    TcpTls(TlsStream<TcpStream>),
}

impl Transport {
    fn into_wg_send_recv(self) -> (WgSender, WgReceiver, Option<(quinn::SendStream, quinn::RecvStream)>, Option<AbortOnDrop>) {
        let tls_stream = match self {
            Transport::Quic { conn, send, recv } => {
                return (
                    WgSender::Quic { conn: conn.clone(), last_send_err_logged_at: None.into() },
                    WgReceiver::Quic(conn),
                    Some((send, recv)),
                    None,
                );
            }
            Transport::TcpTls(tls_stream) => tls_stream,
        };
        let (relay_read, mut relay_write) = io::split(tls_stream);

        let (traffic_state, mut traffic_state_watch) = watch::channel(WgTrafficState::default());
        let sender = WgSender::TcpTls { traffic_state: traffic_state.clone() };
        let receiver = WgReceiver::new_tcp_tls(traffic_state.clone(), relay_read);

        let abort_handle = spawn(async move {
            let mut oks_in_progress: VecDeque<MessageContext> = VecDeque::new();
            let mut packet_in_progress: Option<Vec<u8>> = None;
            loop {
                loop {
                    traffic_state.send_if_modified(|traffic_state| {
                        if !traffic_state.queued_oks.is_empty() {
                            mem::swap(&mut traffic_state.queued_oks, &mut oks_in_progress);
                            true
                        } else if !traffic_state.queued_packets.is_empty() {
                            packet_in_progress = traffic_state.queued_packets.pop_front();
                            true
                        } else {
                            false
                        }
                    });
                    if let Some(packet) = packet_in_progress.take() {
                        if let Err(error) = send_message(
                            &mut relay_write,
                            MessageCode::Op(RelayOpCode::WireGuard),
                            MessageContext::MIN_CLIENT_INITIATED,
                            &packet,
                        )
                        .await
                        {
                            tracing::error!(
                                message_id = "xKBeN0Jb",
                                ?error,
                                "ending tcp tls relay send loop due to WG packet send error"
                            );
                            return;
                        }
                    } else if !oks_in_progress.is_empty() {
                        while let Some(context_id) = oks_in_progress.pop_front() {
                            if let Err(error) = send_message(&mut relay_write, MessageCode::Response(RelayResponseCode::Ok), context_id, &[]).await {
                                tracing::error!(
                                    message_id = "RfElrp6D",
                                    ?error,
                                    "ending tcp tls relay send loop due to response send error"
                                );
                                return;
                            }
                        }
                    } else {
                        break;
                    }
                }
                _ = traffic_state_watch.changed().await;
            }
        })
        .abort_handle()
        .into();

        (sender, receiver, None, Some(abort_handle))
    }
}

enum WgSender {
    Quic {
        conn: quinn::Connection,
        last_send_err_logged_at: Mutex<Option<Instant>>,
    },
    TcpTls {
        traffic_state: watch::Sender<WgTrafficState>,
    },
}

impl WgSender {
    fn send_wg_message(&self, wg_message: &[u8]) {
        match self {
            WgSender::Quic { conn, last_send_err_logged_at } => {
                if let Err(error) = conn.send_datagram(wg_message.to_vec().into()) {
                    // rate-limited logging because this can get VERY noisy and is usually not interesting
                    const SILENCE_SECS: u64 = 1;
                    let mut last_send_err_logged_at = last_send_err_logged_at.lock().unwrap();
                    if !last_send_err_logged_at.is_some_and(|last_log_at| last_log_at.elapsed().as_secs() < SILENCE_SECS) {
                        *last_send_err_logged_at = Some(Instant::now());
                        drop(last_send_err_logged_at);
                        tracing::error!(
                            message_id = "8EkAaj9z",
                            ?error,
                            "error while sending quic datagram packet, silencing this log for {SILENCE_SECS}s"
                        );
                    }
                }
            }
            WgSender::TcpTls { traffic_state } => {
                traffic_state.send_modify(|traffic_state| {
                    if traffic_state.queued_packets.len() < 1000 {
                        traffic_state.queued_packets.push_back(wg_message.to_vec());
                    }
                });
            }
        };
    }
}

enum WgReceiver {
    Quic(quinn::Connection),
    TcpTls {
        traffic_state: watch::Sender<WgTrafficState>,
        #[allow(clippy::type_complexity)]
        recv_message_stream: tokio::sync::Mutex<Pin<Box<dyn Stream<Item = Result<(MessageCode, MessageContext, Vec<u8>), io::Error>> + Send>>>,
    },
}

impl WgReceiver {
    fn new_tcp_tls(traffic_state: watch::Sender<WgTrafficState>, relay_read: ReadHalf<TlsStream<TcpStream>>) -> Self {
        let recv_message_stream = Box::pin(unfold(relay_read, |mut relay_read| async move {
            let item = recv_message(&mut relay_read).await;
            Some((item, relay_read))
        }));
        Self::TcpTls { traffic_state, recv_message_stream: tokio::sync::Mutex::new(recv_message_stream) }
    }

    async fn receive_wg_message(&self) -> io::Result<Bytes> {
        loop {
            return match self {
                WgReceiver::Quic(conn) => conn.read_datagram().await.map_err(io::Error::other),
                WgReceiver::TcpTls { traffic_state, recv_message_stream } => {
                    let (code, context_id, arg) = recv_message_stream.lock().await.next().await.unwrap()?;
                    let op_code = match code {
                        MessageCode::Op(op_code) => {
                            traffic_state.send_modify(|traffic_state| traffic_state.queued_oks.push_back(context_id));
                            op_code
                        }
                        MessageCode::Response(RelayResponseCode::Ok) => continue,
                        MessageCode::Response(RelayResponseCode::Error(error_code)) => {
                            return Err(io::Error::other(RelayErrorResponse::new(error_code, &arg)));
                        }
                    };
                    match op_code {
                        RelayOpCode::WireGuard => Ok(arg.into()),
                        op_code => Err(io::Error::other(UnexpectedOpCode(op_code))),
                    }
                }
            };
        }
    }
}

#[derive(Default)]
struct WgTrafficState {
    queued_oks: VecDeque<MessageContext>,
    queued_packets: VecDeque<Vec<u8>>,
}

async fn send_message<T: AsyncWrite + Unpin>(transport: &mut T, code: MessageCode, context_id: MessageContext, arg: &[u8]) -> Result<(), io::Error> {
    let code = code.to_bytes();
    let msg_header: [u8; 8] = MessageHeader { context_id, payload_length: 4 + arg.len() as u32 }.into();
    transport.write_all(&msg_header).await?;
    transport.write_all(&code).await?;
    transport.write_all(arg).await?;
    transport.flush().await
}

async fn recv_skip<T: AsyncRead + Unpin>(transport: &mut T, mut n: usize) -> Result<(), io::Error> {
    let mut buffer = vec![0u8; u16::MAX.into()];
    while n >= buffer.len() {
        transport.read_exact(&mut buffer).await?;
        n -= buffer.len();
    }
    if n > 0 {
        transport.read_exact(&mut buffer[0..n]).await?;
    }
    Ok(())
}

async fn recv_message<T: AsyncRead + Unpin>(transport: &mut T) -> Result<(MessageCode, MessageContext, Vec<u8>), io::Error> {
    loop {
        let header = MessageHeader::from(recv_fixed::<8, _>(transport).await?);
        let len = header.payload_length_usize();
        if len < 4 || len > u16::MAX as usize + 4 {
            tracing::warn!(message_id = "1gPHoHdA", len, "ignoring relay message with payload too small or large");
            recv_skip(transport, len).await?;
        }
        let mut payload = vec![0u8; len];
        transport.read_exact(&mut payload).await?;
        let (code, arg) = payload.split_at_checked(4).unwrap();
        let code = code.try_into().unwrap_or([u8::MAX; 4]);
        let Some(code) = MessageCode::from_bytes(code, header.context_id, true) else {
            // Forward compatibility with future relay protocol changes
            tracing::warn!(message_id = "OK8fVfBL", "ignoring relay message with unknown op code");
            continue;
        };
        return Ok((code, header.context_id, arg.to_vec()));
    }
}

async fn recv_fixed<const N: usize, T: AsyncRead + Unpin>(transport: &mut T) -> Result<[u8; N], io::Error> {
    let mut buf = [0u8; N];
    transport.read_exact(&mut buf[..]).await?;
    Ok(buf)
}

#[derive(Debug)]
struct VerifyVpnServerCert {
    cert: CertificateDer<'static>,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for VerifyVpnServerCert {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        match self.cert == *end_entity {
            true => Ok(ServerCertVerified::assertion()),
            false => Err(rustls::Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)),
        }
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}
