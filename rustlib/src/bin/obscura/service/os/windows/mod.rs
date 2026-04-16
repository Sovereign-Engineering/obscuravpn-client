pub mod nrpt;
mod start_error;
pub mod tun;

use bytes::Bytes;
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode, ManagerCmdOk};
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::network_config::OsNetworkConfig;
use obscuravpn_client::os::os_trait::Os;
use obscuravpn_client::quicwg::QuicWgConnPacketSender;
pub use start_error::WindowsServiceStartError;
use std::future::pending;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::watch::Receiver;
use tun::Tun;
mod adapters;
mod gaa;
mod iphelper;

pub struct WindowsOsImpl {
    tun: Tun,
    sent_start_command: AtomicBool,
    active_adapter_watcher: Receiver<Option<NetworkInterface>>,
}

impl WindowsOsImpl {
    pub async fn new() -> Result<Self, WindowsServiceStartError> {
        let tun = Tun::create().await?;
        Ok(Self {
            tun,
            sent_start_command: Default::default(),
            active_adapter_watcher: adapters::watch_active_adapter(),
        })
    }

    pub async fn next_manager_command(&self) -> (ManagerCmd, Box<dyn FnOnce(Result<ManagerCmdOk, ManagerCmdErrorCode>) + Send>) {
        if self.sent_start_command.swap(true, Ordering::SeqCst) {
            pending().await
        } else {
            let cmd = ManagerCmd::SetTunnelArgs { args: None, active: Some(true) };
            let result_callback = |result: Result<ManagerCmdOk, ManagerCmdErrorCode>| {
                tracing::info!(message_id = "ZuqhHDfS", "manager called result callback: {:?}", result);
            };
            (cmd, Box::new(result_callback))
        }
    }
}

impl Os for WindowsOsImpl {
    fn network_interface(&self) -> Receiver<Option<NetworkInterface>> {
        self.active_adapter_watcher.clone()
    }

    async fn set_os_network_config(&self, network_config: OsNetworkConfig, tunnel: QuicWgConnPacketSender) -> Result<(), ()> {
        tracing::info!(message_id = "HSSPAPbp", "manager called set_tunnel_network_config: {:?}", network_config);
        let result = self
            .tun
            .set_config(network_config.mtu, network_config.ipv4, network_config.ipv6, Some(network_config.dns))
            .await;
        self.tun.spawn_read_task(tunnel);
        result
    }

    async fn unset_os_network_config(&self) -> Result<(), ()> {
        tracing::info!(message_id = "fPjdNl3o", "manager called unset_tunnel_network_config");
        self.tun.shutdown().await
    }

    fn packet_for_os(&self, packet: Bytes) {
        self.tun.send(packet);
    }
}
