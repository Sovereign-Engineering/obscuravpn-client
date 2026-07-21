mod ipc;
pub mod nrpt;
pub mod scm;
mod start_error;
pub mod tun;

use bytes::Bytes;
use ipc::ServiceIpc;
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode, ManagerCmdOk};
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::network_config::OsNetworkConfig;
use obscuravpn_client::os::os_trait::Os;
use obscuravpn_client::quicwg::QuicWgConnPacketSender;
pub use start_error::WindowsServiceStartError;
use tokio::sync::watch::Receiver;
use tun::Tun;
mod adapters;
mod gaa;
mod iphelper;

pub struct WindowsOsImpl {
    tun: Tun,
    ipc: ServiceIpc,
    active_adapter_watcher: Receiver<Option<NetworkInterface>>,
}

impl WindowsOsImpl {
    pub async fn new() -> Result<Self, WindowsServiceStartError> {
        let tun = Tun::create().await?;
        Ok(Self { tun, active_adapter_watcher: adapters::watch_active_adapter(), ipc: ServiceIpc::new()? })
    }

    pub async fn next_manager_command(&self) -> (ManagerCmd, Box<dyn FnOnce(Result<ManagerCmdOk, ManagerCmdErrorCode>) + Send>) {
        loop {
            let (json_cmd, response_fn) = self.ipc.next().await;
            let response_fn = move |result: Result<ManagerCmdOk, ManagerCmdErrorCode>| {
                let json_response = serde_json::to_vec(&result)
                    .map_err(|error| {
                        tracing::error!(message_id = "zPDkA52Z", ?error, "failed to encode command result");
                        ManagerCmdErrorCode::Other
                    })
                    .unwrap_or(JSON_OTHER_ERROR.into());
                response_fn(json_response)
            };
            match ManagerCmd::from_json(&json_cmd) {
                Ok(cmd) => return (cmd, Box::new(response_fn)),
                Err(error) => response_fn(Err(error)),
            }
        }
    }
}

const JSON_OTHER_ERROR: &str = r#"{"Err":"other"}"#;

#[test]
fn test_other_error_json() {
    assert_eq!(
        serde_json::to_string(&Result::<ManagerCmdOk, ManagerCmdErrorCode>::Err(ManagerCmdErrorCode::Other)).unwrap(),
        JSON_OTHER_ERROR
    )
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
