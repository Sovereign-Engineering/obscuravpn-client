use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::{
    mem,
    net::{IpAddr, Ipv4Addr},
    time::{Duration, Instant},
};

use base64::prelude::*;
use boringtun::x25519::{PublicKey, StaticSecret};
use chrono::Utc;
use obscuravpn_api::types::{AuthToken, OneExit};
use obscuravpn_api::{
    cmd::{ApiErrorKind, Cmd, CreateTunnel, DeleteTunnel, ListRelays, ListTunnels},
    types::{ObfuscatedTunnelConfig, OneRelay, TunnelConfig, WgPubkey},
    Client, ClientError,
};
use quinn::rustls::pki_types::CertificateDer;
use rand::rngs::OsRng;
use tokio::{net::UdpSocket, time::timeout};
use uuid::Uuid;

use crate::config::ConfigSaveError;
use crate::{
    config::{self, Config, ConfigLoadError},
    errors::RelaySelectionError,
    net::{new_quic, new_udp},
    quicwg::QuicWgConn,
};

use super::{
    errors::{ApiError, TunnelConnectError},
    network_config::NetworkConfig,
};

pub struct ClientState {
    user_agent: String,
    inner: Mutex<ClientStateInner>,
}

struct ClientStateInner {
    config_dir: PathBuf,
    config: Config,
    cached_api_client: Option<Arc<Client>>,
}

impl ClientStateInner {
    fn base_url(&self) -> String {
        self.config.api_url.clone().unwrap_or(crate::DEFAULT_API_URL.to_string())
    }
}

impl ClientState {
    pub fn new(config_dir: PathBuf, old_config_dir: PathBuf, user_agent: String) -> Result<Self, ConfigLoadError> {
        let config = config::load(&config_dir, &old_config_dir)?;
        let inner = ClientStateInner { config_dir, config, cached_api_client: None }.into();
        Ok(Self { user_agent, inner })
    }

    fn lock(&self) -> MutexGuard<ClientStateInner> {
        self.inner.lock().unwrap()
    }

    fn change_config(inner: &mut ClientStateInner, f: impl FnOnce(&mut Config)) -> Result<(), ConfigSaveError> {
        let mut new_config = inner.config.clone();
        f(&mut new_config);
        if inner.config != new_config {
            config::save(&inner.config_dir, &new_config)?;
        }
        inner.config = new_config;
        Ok(())
    }

    pub fn set_account_id(&self, account_id: Option<String>, auth_token: Option<AuthToken>) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        inner.cached_api_client = None;
        Self::change_config(&mut inner, move |config| {
            if let Some(old_account_id) = mem::replace(&mut config.account_id, account_id) {
                if !config.old_account_ids.contains(&old_account_id) {
                    config.old_account_ids.push(old_account_id);
                }
            }
            config.cached_auth_token = config.account_id.as_ref().and_then(|_| auth_token.map(Into::into));
        })?;
        Ok(())
    }

    pub fn get_config(&self) -> Config {
        self.lock().config.clone()
    }

    pub fn set_pinned_exits(&self, exits: Vec<String>) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config| config.pinned_exits = exits)?;
        Ok(())
    }

    pub fn set_in_new_account_flow(&self, value: bool) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config| config.in_new_account_flow = value)?;
        Ok(())
    }

    pub fn set_api_url(&self, url: Option<String>) -> Result<(), ConfigSaveError> {
        let mut inner = self.lock();
        Self::change_config(&mut inner, move |config| config.api_url = url)?;
        Ok(())
    }

    pub(crate) async fn connect(&self, exit: Option<String>) -> Result<(QuicWgConn, NetworkConfig, OneExit, OneRelay), TunnelConnectError> {
        let chose_exit = exit.is_some();
        let (token, tunnel_config, wg_sk, exit, relay) = self.new_tunnel(exit).await?;
        let network_config = NetworkConfig::new(&tunnel_config)?;
        tracing::info!(
            tunnel.id =% token,
            exit.pubkey =? tunnel_config.exit_pubkey,
            relay.addr =% tunnel_config.relay_addr_v4,
            "connecting to tunnel");
        let udp = new_udp(None).map_err(TunnelConnectError::UdpSetup)?;
        let quic = new_quic(udp).map_err(TunnelConnectError::QuicSetup)?;
        let remote_pk = PublicKey::from(tunnel_config.exit_pubkey.0);
        let relay_addr = tunnel_config.relay_addr_v4.into();
        let relay_cert = CertificateDer::from(
            BASE64_STANDARD
                .decode(tunnel_config.relay_cert)
                .map_err(|err| TunnelConnectError::InvalidRelayCert(err.into()))?,
        );
        let conn = QuicWgConn::connect(wg_sk.clone(), remote_pk, relay_addr, relay_cert, quic, token).await?;
        tracing::info!("tunnel connected");
        if chose_exit {
            _ = Self::change_config(&mut self.lock(), |config| config.last_chosen_exit = Some(exit.id.clone()));
        }
        Ok((conn, network_config, exit, relay))
    }

    async fn new_tunnel(
        &self,
        exit: Option<String>,
    ) -> anyhow::Result<(Uuid, ObfuscatedTunnelConfig, StaticSecret, OneExit, OneRelay), TunnelConnectError> {
        if let Err(err) = self.remove_local_tunnels().await {
            tracing::warn!("error removing unused local tunnels: {}", err);
        }

        let closest_relay = self.select_relay().await?;

        let sk = StaticSecret::random_from_rng(OsRng);
        let pk = PublicKey::from(&sk);

        let wg_pubkey = WgPubkey(pk.to_bytes());
        let tunnel_id = Uuid::new_v4();
        tracing::info!(
            %tunnel_id,
            client.pubkey =? wg_pubkey,
            exit.id = exit,
            relay.id =? closest_relay.id,
            relay.ip_v4 =% closest_relay.ip_v4,
            relay.ip_v6 =% closest_relay.ip_v6,
            "creating tunnel");

        _ = Self::change_config(&mut self.lock(), |config| config.local_tunnels_ids.push(tunnel_id.to_string()));

        let cmd = CreateTunnel::Obfuscated { id: Some(tunnel_id), wg_pubkey, relay: Some(closest_relay.id), exit };
        let tunnel_info = loop {
            let error = match self.api_request(cmd.clone()).await {
                Ok(t) => break t,
                Err(err) => match err {
                    ApiError::ApiClient(ClientError::ApiError(ref api_error)) => match api_error.body.error {
                        ApiErrorKind::TunnelLimitExceeded {} => err,
                        _ => return Err(err.into()),
                    },
                    err => return Err(err.into()),
                },
            };
            tracing::warn!(?error, "no tunnel slots left, trying to delete an unused one");
            let last_used_threshold = Utc::now().timestamp() - 300;
            let mut tunnels: Vec<(String, i64)> = self
                .api_request(ListTunnels {})
                .await?
                .into_iter()
                .filter_map(|t| match &t.config {
                    TunnelConfig::Obfuscated(_) => {
                        use obscuravpn_api::types::TunnelStatus::*;
                        let (Created { when } | Connected { when } | Disconnected { when }) = t.status;
                        (when < last_used_threshold).then_some((t.id, when))
                    }
                    _ => None,
                })
                .collect();
            tunnels.sort_by_key(|t| t.1);
            let Some(id) = tunnels.into_iter().next().map(|t| t.0) else {
                tracing::warn!("no unused obfuscated tunnel found");
                return Err(error.into());
            };
            tracing::warn!("deleting unused tunnel {}", &id);
            self.api_request(DeleteTunnel { id }).await?;
        };

        let TunnelConfig::Obfuscated(config) = tunnel_info.config else {
            return Err(TunnelConnectError::UnexpectedTunnelKind);
        };
        Ok((tunnel_id, config, sk, tunnel_info.exit, tunnel_info.relay))
    }

    pub async fn remove_local_tunnels(&self) -> Result<(), ApiError> {
        loop {
            let Some(local_tunnel_id) = self.lock().config.local_tunnels_ids.first().cloned() else {
                return Ok(());
            };
            tracing::info!("removing previously used tunnel {}", &local_tunnel_id);
            self.api_request(DeleteTunnel { id: local_tunnel_id.clone() }).await?;
            _ = Self::change_config(&mut self.lock(), |config| config.local_tunnels_ids.retain(|x| x != &local_tunnel_id))
        }
    }

    async fn select_relay(&self) -> Result<OneRelay, TunnelConnectError> {
        let relays = self.api_request(ListRelays {}).await?;
        let udp_socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.map_err(RelaySelectionError::Io)?;
        let send_payload = Uuid::new_v4().into_bytes();
        let start = Instant::now();
        for _ in 0..2 {
            for relay in &relays {
                _ = udp_socket.send_to(&send_payload, (relay.ip_v4, 441)).await;
            }
        }
        let closest_relay = timeout(Duration::from_secs(3), async {
            let mut recv_payload = [0u8; 16];
            loop {
                let (len, addr) = udp_socket.recv_from(&mut recv_payload).await?;
                if len != send_payload.len() || recv_payload != send_payload {
                    continue;
                }
                tracing::info!("received udp echo reponse from after {}ms", start.elapsed().as_millis());
                for relay in &relays {
                    match addr.ip() {
                        IpAddr::V4(ip) if ip == relay.ip_v4 => return Ok(relay.clone()),
                        _ => continue,
                    }
                }
            }
        })
        .await
        .map_err(|_| RelaySelectionError::Timeout)?
        .map_err(RelaySelectionError::Io)?;
        tracing::info!("selected relay {}", closest_relay.id);
        Ok(closest_relay)
    }

    pub async fn api_request<C: Cmd>(&self, cmd: C) -> Result<C::Output, ApiError> {
        let api_client = {
            // MUST NOT BLOCK UNTIL `MutexGuard` IS DROPPED
            let mut inner: MutexGuard<'_, ClientStateInner> = self.lock();
            let Some(account_id) = inner.config.account_id.clone() else {
                return Err(ApiError::NoAccountId);
            };
            if let Some(api_client) = inner.cached_api_client.clone() {
                api_client
            } else {
                let base_url = inner.base_url();
                let api_client = Arc::new(Client::new(base_url, account_id, &self.user_agent).map_err(ClientError::from)?);
                if let Some(auth_token) = inner.config.cached_auth_token.clone() {
                    api_client.set_auth_token(Some(auth_token.into()));
                }
                inner.cached_api_client.insert(api_client).clone()
            }
            // IMPLICITLY DROPPING `MutexGuard`
        };

        let result = api_client.run(cmd).await;

        // MUST NOT BLOCK UNTIL `MutexGuard` IS DROPPED
        let mut inner: MutexGuard<'_, ClientStateInner> = self.lock();
        let auth_token = inner.cached_api_client.clone().and_then(|c| c.get_auth_token());
        Self::change_config(&mut inner, |config| config.cached_auth_token = auth_token.map(Into::into))?;
        drop(inner);
        // DROPPED `MutexGuard`

        Ok(result?)
    }

    pub fn base_url(&self) -> String {
        self.lock().base_url()
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }
}
