pub mod dns;
pub mod ipc;
mod network_manager;
mod routes;
mod service_lock;
pub mod start_error;
pub mod tun;

use crate::DnsManagerArg;
use crate::service::os::Os;
use crate::service::os::linux::dns::{DnsManager, choose_dns_manager, resolved};
use crate::service::os::linux::ipc::ServiceIpc;
use crate::service::os::linux::routes::{ROUTES, netlink};
use crate::service::os::linux::service_lock::ServiceLock;
use crate::service::os::linux::start_error::ServiceStartError;
use crate::service::os::linux::tun::{Tun, TunWriter};
use crate::service::os::packet_buffer::PacketBuffer;
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode, ManagerCmdOk};
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::network_config::TunnelNetworkConfig;
use tokio::sync::watch::Receiver;

pub struct LinuxOsImpl {
    tun: Tun,
    preferred_network_interface: Receiver<Option<NetworkInterface>>,
    current_network_config: Result<Option<TunnelNetworkConfig>, ()>,
    dns_manager_arg: DnsManagerArg,
    ipc: ServiceIpc,
    _lock: ServiceLock,
}

impl LinuxOsImpl {
    pub async fn new(dns_manager_arg: DnsManagerArg) -> Result<Self, ServiceStartError> {
        let lock = ServiceLock::new()?;
        choose_dns_manager(dns_manager_arg).await.map_err(|()| ServiceStartError::NoDnsManager)?;
        let ipc = ServiceIpc::new(&lock).await?;
        Ok(Self {
            _lock: lock,
            ipc,
            tun: Tun::create().await?,
            preferred_network_interface: netlink::watch_preferred_network_interface().await,
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
        loop {
            let (json_cmd, response_fn) = self.ipc.next().await;
            let response_fn = move |result: Result<ManagerCmdOk, ManagerCmdErrorCode>| {
                let json_response = serde_json::to_vec(&result)
                    .map_err(|error| {
                        tracing::error!(message_id = "8Jj0yWQt", ?error, "failed to encode command result: {}", error);
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
