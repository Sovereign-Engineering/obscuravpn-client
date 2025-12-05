pub mod netlink;
mod positive_u31;
mod resolve_d;
pub mod tun;

use crate::service::os::Os;
use crate::service::os::linux::netlink::{add_routes, del_routes, watch_preferred_network_interface};
use crate::service::os::linux::resolve_d::{revert_dns, set_dns};
use crate::service::os::linux::tun::{Tun, TunWriter};
use crate::service::os::packet_buffer::PacketBuffer;
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode, ManagerCmdOk};
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::network_config::TunnelNetworkConfig;
use std::collections::VecDeque;
use std::future;
use tokio::sync::watch::Receiver;

pub struct LinuxOsImpl {
    tun: Tun,
    preferred_network_interface: Receiver<Option<NetworkInterface>>,
    pending_commands: std::sync::Mutex<VecDeque<ManagerCmd>>,
    current_network_config: Result<Option<TunnelNetworkConfig>, ()>,
}

impl LinuxOsImpl {
    pub fn new(runtime: &tokio::runtime::Handle, init_commands: Vec<ManagerCmd>) -> anyhow::Result<Self> {
        Ok(Self {
            tun: Tun::create(runtime)?,
            preferred_network_interface: watch_preferred_network_interface(runtime),
            pending_commands: VecDeque::from(init_commands).into(),
            current_network_config: Ok(None),
        })
    }
}

impl Os for LinuxOsImpl {
    type PutIncomingPacketFn = TunWriter;

    fn network_interface(&self) -> Receiver<Option<NetworkInterface>> {
        self.preferred_network_interface.clone()
    }

    async fn set_tunnel_network_config(&mut self, network_config: TunnelNetworkConfig) -> Result<(), ()> {
        let tun_idx = self.tun.interface_index();
        let mut result = Ok(());
        // Attempt all config steps regardless of individual failures to minimize leaks until intentionally disconnecting. E.g. DNS queries shouldn't because route setup failed.
        result = result.and(self.tun.set_config(network_config.mtu, network_config.ipv4, network_config.ipv6));
        result = result.and(add_routes(tun_idx).await);
        result = result.and(set_dns(tun_idx, &network_config.dns).await);
        self.current_network_config = result.map(|_| Some(network_config));
        result
    }

    async fn unset_tunnel_network_config(&mut self) -> Result<(), ()> {
        let tun_idx = self.tun.interface_index();
        let mut result = Ok(());
        result = result.and(del_routes(tun_idx).await);
        result = result.and(revert_dns(tun_idx).await);
        self.current_network_config = result.map(|_| None);
        result
    }

    fn put_incoming_packet_fn(&self) -> Self::PutIncomingPacketFn {
        self.tun.writer()
    }

    async fn get_outgoing_packets(&self, packet_buffer: &mut PacketBuffer) {
        self.tun.receive(packet_buffer).await;
    }

    async fn get_manager_command(&self) -> (ManagerCmd, Box<dyn FnOnce(Result<ManagerCmdOk, ManagerCmdErrorCode>) + Send>) {
        if let Some(cmd) = self.pending_commands.lock().unwrap().pop_front() {
            return (cmd, Box::new(move |_| {}));
        }
        future::pending().await
    }
}
