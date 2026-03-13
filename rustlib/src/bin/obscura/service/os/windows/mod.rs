mod start_error;
pub mod tun;

use crate::DnsManagerArg;
use crate::service::os::Os;
use crate::service::os::packet_buffer::PacketBuffer;
use crate::service::os::windows::tun::TunWriter;
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode, ManagerCmdOk};
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::network_config::TunnelNetworkConfig;
pub use start_error::WindowsServiceStartError;
use std::future::pending;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::watch::Receiver;
use tun::Tun;

pub struct WindowsOsImpl {
    tun: Tun,
    sent_start_command: AtomicBool,
}

impl WindowsOsImpl {
    pub async fn new() -> Result<Self, WindowsServiceStartError> {
        let tun = Tun::new();
        Ok(Self { tun, sent_start_command: Default::default() })
    }
}

impl Os for WindowsOsImpl {
    type PutIncomingPacketFn = TunWriter;

    fn network_interface(&self) -> Receiver<Option<NetworkInterface>> {
        tokio::sync::watch::channel(None).1
    }

    async fn set_tunnel_network_config(&mut self, network_config: OsNetworkConfig) -> Result<(), ()> {
        tracing::info!("manager called set_tunnel_network_config: {:?}", network_config);
        Ok(())
    }

    async fn unset_tunnel_network_config(&mut self) -> Result<(), ()> {
        tracing::info!("manager called unset_tunnel_network_config");
        Ok(())
    }

    fn put_incoming_packet_fn(&self) -> Self::PutIncomingPacketFn {
        self.tun.writer()
    }

    async fn get_outgoing_packets(&self, packet_buffer: &mut PacketBuffer) {
        pending().await
    }

    async fn get_manager_command(&self) -> (ManagerCmd, Box<dyn FnOnce(Result<ManagerCmdOk, ManagerCmdErrorCode>) + Send>) {
        if self.sent_start_command.swap(true, Ordering::SeqCst) {
            pending().await
        } else {
            let cmd = ManagerCmd::SetTunnelArgs { args: None, active: Some(true) };
            let result_callback = |result: Result<ManagerCmdOk, ManagerCmdErrorCode>| {
                tracing::info!("manager called result callback: {:?}", result);
            };
            (cmd, Box::new(result_callback))
        }
    }
}
