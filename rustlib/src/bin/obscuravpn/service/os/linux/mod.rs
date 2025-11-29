pub mod netlink;
pub mod tun;

use crate::service::os::Os;
use crate::service::os::linux::netlink::{add_routes, del_routes, watch_preferred_network_interface};
use crate::service::os::linux::tun::{Tun, TunWriter};
use crate::service::os::packet_buffer::PacketBuffer;
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode, ManagerCmdOk};
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::network_config::TunnelNetworkConfig;
use std::collections::VecDeque;
use std::future;
use std::sync::Arc;
use tokio::sync::watch::Receiver;

pub struct LinuxOsImpl {
    runtime: tokio::runtime::Handle,
    route_setting_lock: Arc<tokio::sync::Mutex<()>>,
    tun: Tun,
    preferred_network_interface: Receiver<Option<NetworkInterface>>,
    pending_commands: std::sync::Mutex<VecDeque<ManagerCmd>>,
}

impl LinuxOsImpl {
    pub fn new(runtime: &tokio::runtime::Handle, init_commands: Vec<ManagerCmd>) -> anyhow::Result<Self> {
        Ok(Self {
            tun: Tun::create(runtime)?,
            preferred_network_interface: watch_preferred_network_interface(runtime),
            runtime: runtime.clone(),
            route_setting_lock: Default::default(),
            pending_commands: VecDeque::from(init_commands).into(),
        })
    }
}

impl Os for LinuxOsImpl {
    type PutIncomingPacketFn = TunWriter;

    fn network_interface(&self) -> Receiver<Option<NetworkInterface>> {
        self.preferred_network_interface.clone()
    }

    async fn set_tunnel_network_config(&mut self, network_config: TunnelNetworkConfig) -> Result<(), ()> {
        let mut result = Ok(());
        result = result.and(self.tun.set_network_config(network_config));
        tracing::info!(message_id = "0hgQJdfV", "acquiring lock before adding routes");
        let route_setting_guard = self.route_setting_lock.lock().await;
        tracing::info!(message_id = "k1E5gIKk", "acquired lock for adding routes");
        result = result.and(add_routes(self.tun.interface_index()).await);
        drop(route_setting_guard);
        tracing::info!(message_id = "5ysNC9FN", "released lock after adding routes");
        result
    }

    fn unset_tunnel_network_config(&mut self) {
        let route_setting_lock = self.route_setting_lock.clone();
        let tun_interface_index = self.tun.interface_index();
        tracing::info!("entered unset_tunnel_network_config");
        self.runtime.spawn(async move {
            tracing::info!(message_id = "CNWiQHGb", "acquiring lock before removing routes");
            let route_setting_guard = route_setting_lock.lock().await;
            tracing::info!(message_id = "6TJinLz1", "acquired lock for removing routes");
            let _: Result<(), ()> = del_routes(tun_interface_index).await;
            drop(route_setting_guard);
            tracing::info!(message_id = "GkFEiHuc", "released lock after removing routes");
        });
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
