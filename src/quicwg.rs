use boringtun::noise::{Tunn, TunnResult};
use boringtun::x25519::{PublicKey, StaticSecret};
use etherparse::{IcmpEchoHeader, Icmpv4Type, PacketBuilder, SlicedPacket, TransportSlice};
use futures::select_biased;
use futures::FutureExt;
use quinn::crypto::rustls::QuicClientConfig;
use quinn::rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use quinn::rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use quinn::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use quinn::rustls::{CertificateError, DigitallySignedStruct, SignatureScheme};
use quinn::{rustls, ClientConfig, MtuDiscoveryConfig, RecvStream};
use rand::random;
use std::net::{Ipv4Addr, SocketAddr};
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::spawn;
use tokio::sync::Notify;
use tokio::task::AbortHandle;
use tokio::time::{interval, sleep, timeout, MissedTickBehavior};
use tokio::time::{Duration, Instant};
use uuid::Uuid;

// Note that there is a race condition between client connection and the relay learning about our tunnel. In most cases this delay can't be more than a few seconds. So we make sure that we have at least 10s of retries here to cover this case.
const QUIC_CONNECT_RETRY_COUNT: usize = 20;
const QUIC_CONNECT_RETRY_DELAY: Duration = Duration::from_millis(500);
const QUIC_MAX_IDLE_MS: u32 = 5000;

// The primary reason for these retries is that Mullvad's key propagation to exits is sometimes slow. Handshakes will be ignored until the key is known so we need to keep retrying.
// Based on a day of measurements on a good network connection while 97% of handshakes will succeed within 10s we need to wait 21.2s to get a 99% success rate.
// Importantly we have never seen a complete propagation failure, just delays. So it is better to wait and succeed than to cancel.
// A better solution is being tracked in https://linear.app/soveng/issue/OBS-824
const WG_FIRST_HANDSHAKE_RETRIES: usize = 9; // 22.5s total.
const WG_FIRST_HANDSHAKE_RESENDS: usize = 25; // 2.5s per handshake.
const WG_FIRST_HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(100);
const RELAY_SNI: &str = "relay.obscura.net";
const WG_MAX_IDLE_MS: u32 = QUIC_MAX_IDLE_MS;
const WG_TICK_MS: u32 = 1000;

#[derive(Debug, Error)]
pub enum QuicWgReceiveError {
    #[error("wireguard idle timeout")]
    WireguardIdleTimeout,
    #[error("quic send error: {0}")]
    QuicSendError(#[from] quinn::SendDatagramError),
    #[error("quic receive error: {0}")]
    QuicReceiveError(#[from] quinn::ConnectionError),
}

#[derive(Debug, Error)]
pub enum QuicWgConnectError {
    #[error("crypto config: {0}")]
    CryptoConfig(anyhow::Error),
    #[error("quic config: {0}")]
    QuicConfig(#[from] quinn::ConnectError),
    #[error("quic connect: {0}")]
    QuicConnect(#[from] quinn::ConnectionError),
    #[error("relay handshake: {0}")]
    RelayHandshake(#[from] QuicWgRelayHandshakeError),
    #[error("wireguard handshake: {0}")]
    WireguardHandshake(#[from] QuicWgWireguardHandshakeError),
}

#[derive(Debug, Error)]
pub enum QuicWgRelayHandshakeError {
    #[error("could not open control stream: {0}")]
    ControlStreamInitError(#[from] quinn::ConnectionError),
    #[error("could not read ack from control stream: {0}")]
    NoAckReceived(#[from] quinn::ReadExactError),
    #[error("could not write to control stream: {0}")]
    ControlStreamWriteError(#[from] quinn::WriteError),
    #[error("received invalid ack (non-zero: {0})")]
    ReceivedInvalidAck(u8),
}

#[derive(Debug, Error)]
pub enum QuicWgWireguardHandshakeError {
    #[error("could not construct inititialization message")]
    InitMessageConstructError,
    #[error("could not send inititialization message: {0}")]
    InitMessageSendError(#[from] quinn::SendDatagramError),
    #[error("could not receive response message: {0}")]
    RespMessageReceiveError(#[from] quinn::ConnectionError),
    #[error("response timeout")]
    RespMessageTimeout,
}

pub struct QuicWgConn {
    wg_state: Mutex<WgState>,
    quic: quinn::Connection,
    client_public_key: PublicKey,
    exit_public_key: PublicKey,
    client_ip_v4: Ipv4Addr,
    ping_keepalive_ip_v4: Ipv4Addr,
    ping_keepalive_payload: [u8; 16],
    connected_at: Instant,
    wg_tick_notify: Arc<Notify>,
    tick_abort: AbortHandle,
}

#[derive(Default, Clone, Copy)]
pub struct QuicWgTrafficStats {
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub latest_latency_ms: u16,
}

struct WgState {
    buffer: Vec<u8>,
    wg: Tunn,
    traffic_stats: QuicWgTrafficStats,
    ticks_since_last_packet: u32,
    last_send_err_logged_at: Option<Instant>,
}

impl QuicWgConn {
    #[allow(clippy::too_many_arguments)]
    pub async fn connect(
        client_secret_key: StaticSecret,
        exit_public_key: PublicKey,
        relay_addr: SocketAddr,
        relay_cert: CertificateDer<'static>,
        endpoint: quinn::Endpoint,
        client_ip_v4: Ipv4Addr,
        ping_keepalive_ip_v4: Ipv4Addr,
        token: Uuid,
    ) -> Result<Self, QuicWgConnectError> {
        let client_public_key = PublicKey::from(&client_secret_key);
        tracing::info!("connecting to relay");
        let quic = Self::relay_connect(&endpoint, relay_addr, relay_cert, token).await?;
        tracing::info!("connected to relay");

        let index = random();
        let mut wg = Tunn::new(client_secret_key, exit_public_key, None, None, index, None).unwrap();
        Self::first_wg_handshake(&mut wg, &quic, WG_FIRST_HANDSHAKE_RETRIES, WG_FIRST_HANDSHAKE_RESENDS).await?;
        tracing::info!("connected to exit");
        let wg_tick_notify = Arc::new(Notify::new());
        let wg_tick_notify_clone = wg_tick_notify.clone();
        let tick_abort = spawn(async move {
            let mut timer = interval(Duration::from_millis(WG_TICK_MS.into()));
            timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                timer.tick().await;
                wg_tick_notify_clone.notify_one();
            }
        })
        .abort_handle();
        let now = Instant::now();
        let wg_state = Mutex::new(WgState {
            wg,
            traffic_stats: QuicWgTrafficStats::default(),
            buffer: vec![0u8; u16::MAX as usize],
            ticks_since_last_packet: 0,
            last_send_err_logged_at: None,
        });
        Ok(Self {
            quic,
            wg_state,
            client_public_key,
            exit_public_key,
            client_ip_v4,
            ping_keepalive_ip_v4,
            ping_keepalive_payload: random(),
            connected_at: now,
            wg_tick_notify,
            tick_abort,
        })
    }

    async fn relay_connect(
        endpoint: &quinn::Endpoint,
        relay_addr: SocketAddr,
        relay_cert: CertificateDer<'static>,
        token: Uuid,
    ) -> Result<quinn::Connection, QuicWgConnectError> {
        let quic_config = Self::quic_config(relay_cert)?;
        let mut retries = QUIC_CONNECT_RETRY_COUNT;
        loop {
            match Self::try_relay_connect(endpoint, &quic_config, relay_addr, token).await {
                Ok(quic) => return Ok(quic),
                Err(error) => match error {
                    QuicWgConnectError::RelayHandshake(QuicWgRelayHandshakeError::NoAckReceived(_)) => {
                        tracing::info!(?error, "relay handshake failed at ack, relay may not know the token yet");
                        if retries == 0 {
                            return Err(error);
                        }
                        retries -= 1;
                        tracing::info!("will retry relay connect again in {:?}", QUIC_CONNECT_RETRY_DELAY);
                        sleep(QUIC_CONNECT_RETRY_DELAY).await
                    }
                    _ => return Err(error),
                },
            }
        }
    }

    fn quic_config(relay_cert: CertificateDer<'static>) -> Result<ClientConfig, QuicWgConnectError> {
        let default_provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut crypto = rustls::ClientConfig::builder_with_provider(default_provider.clone())
            .with_safe_default_protocol_versions()
            .map_err(|error| QuicWgConnectError::CryptoConfig(error.into()))?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(VerifyVpnServerCert { cert: relay_cert, provider: default_provider }))
            .with_no_client_auth();
        crypto.alpn_protocols = vec![b"h3".to_vec()];
        let crypto = QuicClientConfig::try_from(crypto).map_err(|error| QuicWgConnectError::CryptoConfig(error.into()))?;
        let mut client_cfg = ClientConfig::new(Arc::new(crypto));
        let mut transport_config = quinn::TransportConfig::default();
        transport_config.max_concurrent_uni_streams(0u8.into());
        transport_config.max_concurrent_bidi_streams(0u8.into());
        const MTU: u16 = 1350;
        transport_config.initial_mtu(MTU);
        transport_config.min_mtu(MTU);
        let mut mtu_discovery_config = MtuDiscoveryConfig::default();
        mtu_discovery_config.upper_bound(MTU);
        transport_config.mtu_discovery_config(Some(mtu_discovery_config));
        transport_config.keep_alive_interval(Some(Duration::from_secs(1)));
        transport_config.max_idle_timeout(Some(quinn::VarInt::from_u32(QUIC_MAX_IDLE_MS).into()));
        transport_config.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
        client_cfg.transport_config(Arc::new(transport_config));
        Ok(client_cfg)
    }

    async fn try_relay_connect(
        endpoint: &quinn::Endpoint,
        quic_config: &ClientConfig,
        relay_addr: SocketAddr,
        token: Uuid,
    ) -> Result<quinn::Connection, QuicWgConnectError> {
        let quic = endpoint
            .connect_with(quic_config.clone(), relay_addr, RELAY_SNI)
            .map_err(QuicWgConnectError::QuicConfig)?
            .await?;
        let (snd, recv) = quic.open_bi().await?;
        Self::relay_handshake(snd, recv, token).await?;
        Ok(quic)
    }

    async fn relay_handshake(mut snd: quinn::SendStream, mut recv: RecvStream, token: Uuid) -> Result<(), QuicWgRelayHandshakeError> {
        // Protocol version
        snd.write_all(&[2]).await?;
        // Token
        snd.write_all(token.as_bytes()).await?;

        let mut status = [0u8];
        recv.read_exact(&mut status).await?;
        if status[0] != 0 {
            return Err(QuicWgRelayHandshakeError::ReceivedInvalidAck(status[0]));
        }
        Ok(())
    }

    pub fn max_datagram_size(&self) -> Option<usize> {
        self.quic.max_datagram_size()
    }

    async fn receive_quic(&self) -> Result<bytes::Bytes, quinn::ConnectionError> {
        let data = self.quic.read_datagram().await?;
        Ok(data)
    }

    fn build_first_wg_handshake_init(wg: &mut Tunn) -> Result<Vec<u8>, QuicWgWireguardHandshakeError> {
        let mut buf = vec![0u8; u16::MAX as usize];
        let data = match wg.format_handshake_initiation(&mut buf, true) {
            TunnResult::WriteToNetwork(data) => data.to_vec(),
            _ => return Err(QuicWgWireguardHandshakeError::InitMessageConstructError),
        };
        Ok(data)
    }

    fn build_ping_keepalive_packet(&self) -> Vec<u8> {
        let id = random();
        let seq = self.connected_at.elapsed().as_millis() as u16;
        let builder = PacketBuilder::ipv4(self.client_ip_v4.octets(), self.ping_keepalive_ip_v4.octets(), 255).icmpv4_echo_request(id, seq);
        let mut packet = Vec::<u8>::with_capacity(builder.size(self.ping_keepalive_payload.len()));
        builder.write(&mut packet, &self.ping_keepalive_payload).unwrap();
        packet
    }

    fn latency_ms_from_pong_keepalive_packet(&self, packet: &[u8]) -> Option<u16> {
        let ip = SlicedPacket::from_ip(packet).ok()?;
        let Some(TransportSlice::Icmpv4(icmp)) = ip.transport else { return None };
        let Icmpv4Type::EchoReply(IcmpEchoHeader { seq, .. }) = icmp.icmp_type() else {
            return None;
        };
        if icmp.payload() == self.ping_keepalive_payload {
            return Some((self.connected_at.elapsed().as_millis() as u16).wrapping_sub(seq));
        }
        None
    }

    async fn wait_for_first_handshake_response(wg: &mut Tunn, quic: &quinn::Connection) -> Result<(), QuicWgWireguardHandshakeError> {
        let mut buf = vec![0u8; u16::MAX as usize];
        timeout(WG_FIRST_HANDSHAKE_TIMEOUT, async {
            while wg.time_since_last_handshake().is_none() {
                let mut datagram = quic
                    .read_datagram()
                    .await
                    .map_err(QuicWgWireguardHandshakeError::RespMessageReceiveError)?;
                loop {
                    let res = wg.decapsulate(None, &datagram, &mut buf);
                    match Self::handle_result(quic, res)? {
                        ControlFlow::Continue(()) => {
                            datagram.truncate(0);
                            continue;
                        }
                        ControlFlow::Break(Some(_)) => tracing::warn!("unexpected packet during first WG handshake"),
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
        quic: &quinn::Connection,
        mut retries: usize,
        resends: usize,
    ) -> Result<(), QuicWgWireguardHandshakeError> {
        loop {
            retries -= 1;
            let handshake_init = Self::build_first_wg_handshake_init(wg)?;
            let mut resends = resends;
            loop {
                resends -= 1;
                quic.send_datagram(handshake_init.clone().into())?;
                match Self::wait_for_first_handshake_response(wg, quic).await {
                    Ok(()) => return Ok(()),
                    Err(err) => match err {
                        QuicWgWireguardHandshakeError::RespMessageTimeout => {
                            tracing::info!("exit handshake timeout, packet may have gotten lost");
                            if resends == 0 {
                                tracing::info!("too many exit handshake resend attempts, exit may not be configured");
                                break;
                            }
                        }
                        err => return Err(err),
                    },
                }
            }
            if retries == 0 {
                return Err(QuicWgWireguardHandshakeError::RespMessageTimeout);
            }
        }
    }

    fn handle_result(quic: &quinn::Connection, res: TunnResult<'_>) -> Result<ControlFlow<Option<Vec<u8>>>, quinn::SendDatagramError> {
        match res {
            TunnResult::Done => Ok(ControlFlow::Break(None)),
            TunnResult::WriteToNetwork(datagram) => match quic.send_datagram(datagram.to_vec().into()) {
                Ok(()) => Ok(ControlFlow::Continue(())),
                Err(err) => Err(err),
            },
            TunnResult::WriteToTunnelV4(packet, ..) | TunnResult::WriteToTunnelV6(packet, ..) => Ok(ControlFlow::Break(Some(packet.to_vec()))),
            TunnResult::Err(error) => {
                tracing::warn!(?error, "wireguard error");
                Ok(ControlFlow::Break(None))
            }
        }
    }

    pub fn send(&self, packet: &[u8]) {
        let mut wg_state = self.wg_state.lock().unwrap();
        let WgState { buffer, wg, traffic_stats, last_send_err_logged_at, .. } = &mut *wg_state;
        traffic_stats.tx_bytes += packet.len() as u64;
        let res = wg.encapsulate(packet, buffer);
        if let Err(error) = Self::handle_result(&self.quic, res) {
            // rate-limited logging because this can get VERY noisy and is usually not interesting
            const SILENCE_SECS: u64 = 1;
            if !last_send_err_logged_at.is_some_and(|last_log_at| last_log_at.elapsed().as_secs() < SILENCE_SECS) {
                tracing::error!(?error, "error while sending packet, silencing this log for {SILENCE_SECS}min");
                *last_send_err_logged_at = Some(Instant::now());
            }
        }
    }

    pub async fn receive(&self) -> Result<Vec<u8>, QuicWgReceiveError> {
        // TODO: implement QUIC recovery and detect WG interruptions fast (OBS-274)
        loop {
            select_biased! {
                _ = self.wg_tick_notify.notified().fuse() => {
                    let mut wg_state = self.wg_state.lock().unwrap();
                    let WgState {buffer,wg, ticks_since_last_packet, .. } = &mut *wg_state;
                    if ticks_since_last_packet.saturating_mul(WG_TICK_MS) > WG_MAX_IDLE_MS {
                        tracing::error!("no packets received for at least {WG_MAX_IDLE_MS}ms");
                        return Err(QuicWgReceiveError::WireguardIdleTimeout)
                    }
                    loop {
                        let timer_result = wg.update_timers(buffer);
                        match Self::handle_result(&self.quic, timer_result)? {
                            ControlFlow::Continue(()) => continue,
                            ControlFlow::Break(Some(_)) => tracing::warn!("unexpected packet during update_timers"),
                            ControlFlow::Break(None) => break,
                        }
                    }
                    let ping_packet = self.build_ping_keepalive_packet();
                    let ping_result = wg.encapsulate(&ping_packet, buffer);
                    Self::handle_result(&self.quic, ping_result)?;
                    *ticks_since_last_packet += 1;
                }
                receive_quic = self.receive_quic().fuse() => {
                    let mut datagram = receive_quic?;
                    let mut wg_state = self.wg_state.lock().unwrap();
                    let WgState {buffer,wg, ticks_since_last_packet, traffic_stats, .. } = &mut *wg_state;
                    loop {
                        let res = wg.decapsulate(None, &datagram, buffer);
                        match Self::handle_result(&self.quic, res)? {
                            ControlFlow::Continue(()) => {
                                datagram.truncate(0);
                                continue
                            }
                            ControlFlow::Break(Some(packet)) => {
                                *ticks_since_last_packet = 0;
                                traffic_stats.rx_bytes += packet.len() as u64;
                                if let Some(latest_latency_ms) = self.latency_ms_from_pong_keepalive_packet(&packet) {
                                    tracing::info!("received keepalive pong after {}ms", latest_latency_ms);
                                    traffic_stats.latest_latency_ms = latest_latency_ms;
                                    break
                                }
                                return Ok(packet)
                            },
                            ControlFlow::Break(None) => break,
                        }
                    }
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
}

impl Drop for QuicWgConn {
    fn drop(&mut self) {
        self.tick_abort.abort();
    }
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
        match &self.cert == end_entity {
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
