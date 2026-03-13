use super::tunnel::Tun;
use crate::{net::NetworkInterface, network_config::OsNetworkConfig, os::os_trait::Os};
use bytes::Bytes;
use jni::JavaVM;
use std::{os::fd::OwnedFd, sync::Mutex};
use tokio::sync::{oneshot, watch};

pub(super) type SetNetworkConfigSender = oneshot::Sender<Result<OwnedFd, ()>>;

pub struct AndroidOsImpl {
    tun: Mutex<Option<Tun>>,
    network_interface: watch::Sender<Option<NetworkInterface>>,
    jvm: JavaVM,
}

impl AndroidOsImpl {
    pub fn new(jvm: JavaVM) -> Self {
        Self { tun: Mutex::new(None), network_interface: watch::channel(None).0, jvm }
    }

    pub fn set_network_interface(&self, network_interface: Option<NetworkInterface>) {
        self.network_interface.send_replace(network_interface);
    }
}

impl Os for AndroidOsImpl {
    fn network_interface(&self) -> watch::Receiver<Option<NetworkInterface>> {
        self.network_interface.subscribe()
    }

    async fn set_os_network_config(&self, network_config: OsNetworkConfig) -> Result<(), ()> {
        let json = serde_json::to_string(&network_config).map_err(|error| {
            tracing::error!(message_id = "dK2xNm3q", ?error, "failed to serialize OsNetworkConfig: {error}");
        })?;

        let (tx, rx): (SetNetworkConfigSender, _) = oneshot::channel();
        super::ffi::call_set_network_config(&self.jvm, &json, tx)?;

        let fd = match rx.await {
            Ok(Ok(fd)) => Ok(fd),
            Ok(Err(())) => {
                tracing::error!(message_id = "Ivp77dfC", "setting Android network config failed");
                Err(())
            }
            Err(_) => {
                tracing::error!(message_id = "qR8bTc4u", "SetNetworkConfigSender dropped without send");
                Err(())
            }
        }?;

        let manager = super::ffi::global()
            .map_err(|error| tracing::error!(message_id = "zGi10N5H", ?error, "failed to get manager: {error}"))?
            .manager
            .clone();
        let (tun, result) = match Tun::spawn(fd, manager) {
            Ok(tun) => {
                tracing::info!(
                    message_id = "mLrplF1x",
                    ?network_config,
                    "successfully set network config and spawned TUN device"
                );
                (tun, Ok(()))
            }
            Err(tun) => {
                tracing::error!(message_id = "rS9cUd5v", "failed to spawn TUN reader");
                (tun, Err(()))
            }
        };
        *self.tun.lock().unwrap() = Some(tun);
        result
    }

    async fn unset_os_network_config(&self) -> Result<(), ()> {
        *self.tun.lock().unwrap() = None;
        Ok(())
    }

    fn packet_for_os(&self, packet: Bytes) {
        if let Some(tun) = &*self.tun.lock().unwrap() {
            tun.write(&packet);
        }
    }
}
