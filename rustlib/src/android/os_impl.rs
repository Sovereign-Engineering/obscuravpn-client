use super::{class_cache::ClassCache, tunnel::Tun};
use crate::quicwg::QuicWgConnPacketSender;
use crate::{net::NetworkInterface, network_config::OsNetworkConfig, os::os_trait::Os};
use bytes::Bytes;
use jni::JavaVM;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

pub struct AndroidOsImpl {
    tun: Mutex<Option<Tun>>,
    network_interface: watch::Sender<Option<NetworkInterface>>,
    jvm: Arc<JavaVM>,
    class_cache: Arc<ClassCache>,
}

impl AndroidOsImpl {
    pub fn new(jvm: Arc<JavaVM>, class_cache: Arc<ClassCache>) -> Self {
        Self { tun: Mutex::new(None), network_interface: watch::channel(None).0, jvm, class_cache }
    }

    pub fn set_network_interface(&self, network_interface: Option<NetworkInterface>) {
        self.network_interface.send_replace(network_interface);
    }
}

impl Os for AndroidOsImpl {
    fn network_interface(&self) -> watch::Receiver<Option<NetworkInterface>> {
        self.network_interface.subscribe()
    }

    async fn set_os_network_config(&self, network_config: OsNetworkConfig, tunnel: QuicWgConnPacketSender) -> Result<(), ()> {
        let json = serde_json::to_string(&network_config).map_err(|error| {
            tracing::error!(message_id = "dK2xNm3q", ?error, "failed to serialize OsNetworkConfig: {error}");
        })?;

        let fd = super::ffi::call_set_network_config(self.class_cache.clone(), self.jvm.clone(), json).await?;

        let (tun, result) = match Tun::spawn(fd, tunnel) {
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
