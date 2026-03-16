pub mod dns;
pub mod ipc;
mod network_manager;
mod routes;
mod service_lock;
pub mod start_error;
pub mod tun;

use crate::service::os::linux::dns::{DnsManager, DnsManagerArg, choose_dns_manager, resolved};
use crate::service::os::linux::ipc::ServiceIpc;
use crate::service::os::linux::routes::{ROUTES, netlink};
use crate::service::os::linux::service_lock::ServiceLock;
use crate::service::os::linux::tun::Tun;
use bytes::Bytes;
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode, ManagerCmdOk};
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::network_config::OsNetworkConfig;
use obscuravpn_client::os::os_trait::Os;
use obscuravpn_client::quicwg::QuicWgConnPacketSender;
pub use start_error::LinuxServiceStartError;
use tokio::sync::watch::Receiver;

pub struct LinuxOsImpl {
    tun: Tun,
    preferred_network_interface: Receiver<Option<NetworkInterface>>,
    current_network_config: tokio::sync::Mutex<Result<Option<OsNetworkConfig>, ()>>,
    dns_manager_arg: DnsManagerArg,
    ipc: ServiceIpc,
    _lock: ServiceLock,
}

impl LinuxOsImpl {
    pub async fn new(dns_manager_arg: DnsManagerArg) -> Result<Self, LinuxServiceStartError> {
        let lock = ServiceLock::new()?;
        choose_dns_manager(dns_manager_arg)
            .await
            .map_err(|()| LinuxServiceStartError::NoDnsManager)?;
        let ipc = ServiceIpc::new(&lock).await?;
        Ok(Self {
            _lock: lock,
            ipc,
            tun: Tun::create().await?,
            preferred_network_interface: netlink::watch_preferred_network_interface().await,
            current_network_config: Ok(None).into(),
            dns_manager_arg,
        })
    }
}

impl Os for LinuxOsImpl {
    fn network_interface(&self) -> Receiver<Option<NetworkInterface>> {
        self.preferred_network_interface.clone()
    }

    async fn set_os_network_config(&self, network_config: OsNetworkConfig, tunnel: QuicWgConnPacketSender) -> Result<(), ()> {
        let mut current_network_config = self.current_network_config.lock().await;
        let tun = self.tun.interface();

        // Attempt all config steps regardless of individual failures to minimize leaks until intentionally disconnecting. E.g. DNS queries shouldn't leak because route setup failed.
        let mut result = Ok(());
        match choose_dns_manager(self.dns_manager_arg).await? {
            DnsManager::NetworkManager => result = result.and(network_manager::set_dns_and_routes(&tun, &network_config, &ROUTES).await),
            dns_manager => {
                result = result.and(netlink::add_routes(&tun, &ROUTES).await);
                if dns_manager.is_resolved() {
                    match &network_config.dns {
                        Some(dns) => result = result.and(resolved::set_dns(&tun, dns).await),
                        None => result = result.and(resolved::reset_dns(&tun).await),
                    }
                }
            }
        }
        result = result.and(self.tun.set_config(network_config.mtu, network_config.ipv4, network_config.ipv6));
        *current_network_config = result.map(|_| Some(network_config));

        self.tun.spawn_read_task(tunnel);
        result
    }

    async fn unset_os_network_config(&self) -> Result<(), ()> {
        let mut current_network_config = self.current_network_config.lock().await;
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
        *current_network_config = result.map(|_| None);
        result
    }

    fn packet_for_os(&self, packet: Bytes) {
        self.tun.send(packet)
    }
}

impl LinuxOsImpl {
    /// Returns next manager command. Blocks until a command is available. The response function is called with the command result.
    pub async fn next_manager_command(&self) -> (ManagerCmd, Box<dyn FnOnce(Result<ManagerCmdOk, ManagerCmdErrorCode>) + Send>) {
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
