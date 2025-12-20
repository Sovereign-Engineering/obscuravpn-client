pub mod dns;
mod network_manager;
mod routes;
pub mod tun;

use crate::DnsManagerArg;
use crate::service::os::Os;
use crate::service::os::linux::dns::{DnsManager, choose_dns_manager, resolved};
use crate::service::os::linux::routes::{ROUTES, netlink};
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
    dns_manager_arg: DnsManagerArg,
}

impl LinuxOsImpl {
    pub fn new(runtime: &tokio::runtime::Handle, init_commands: Vec<ManagerCmd>, dns_manager_arg: DnsManagerArg) -> anyhow::Result<Self> {
        runtime
            .block_on(choose_dns_manager(dns_manager_arg))
            .map_err(|()| anyhow::anyhow!("failed to detect compatible dns management service"))?;
        Ok(Self {
            tun: Tun::create(runtime)?,
            preferred_network_interface: netlink::watch_preferred_network_interface(runtime),
            pending_commands: VecDeque::from(init_commands).into(),
            current_network_config: Ok(None),
            dns_manager_arg,
        })
    }
}

impl Os for LinuxOsImpl {
    type PutIncomingPacketFn = TunWriter;

    fn network_interface(&self) -> Receiver<Option<NetworkInterface>> {
        self.preferred_network_interface.clone()
    }

    async fn set_tunnel_network_config(&mut self, network_config: TunnelNetworkConfig) -> Result<(), ()> {
        let tun = self.tun.interface();

        // Attempt all config steps regardless of individual failures to minimize leaks until intentionally disconnecting. E.g. DNS queries shouldn't leak because route setup failed.
        let mut result = Ok(());
        match choose_dns_manager(self.dns_manager_arg).await? {
            DnsManager::NetworkManager => result = result.and(network_manager::set_dns_and_routes(&tun, &network_config, &ROUTES).await),
            dns_manager => {
                result = result.and(netlink::add_routes(&tun, &ROUTES).await);
                if dns_manager.is_resolved() {
                    result = result.and(resolved::set_dns(&tun, &network_config.dns).await);
                }
            }
        }
        result = result.and(self.tun.set_config(network_config.mtu, network_config.ipv4, network_config.ipv6));
        self.current_network_config = result.map(|_| Some(network_config));
        result
    }

    async fn unset_tunnel_network_config(&mut self) -> Result<(), ()> {
        let tun = self.tun.interface();
        let mut result = Ok(());
        match choose_dns_manager(self.dns_manager_arg).await? {
            DnsManager::NetworkManager => result = result.and(network_manager::reset_dns_and_routes(&tun).await),
            dns_manager => {
                result = result.and(netlink::del_routes(&tun, &ROUTES).await);
                if dns_manager.is_resolved() {
                    result = result.and(resolved::reset_dns(&tun).await);
                }
            }
        }
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
