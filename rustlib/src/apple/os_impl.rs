use std::ffi::c_void;

use bytes::Bytes;
use tokio::sync::{oneshot, watch};

use crate::ffi_helpers::*;
use crate::net::NetworkInterface;
use crate::network_config::OsNetworkConfig;
use crate::os::os_trait::Os;

pub type SetNetworkConfigCb =
    extern "C" fn(network_config_json: FfiBytes, context: *mut c_void, done: extern "C" fn(context: *mut c_void, success: bool));

/// Callback invoked by Swift when the async `setNetworkConfig` operation completes.
/// Resolves the oneshot sender stored at `context`.
///
/// SAFETY:
/// - `context` must be a value previously provided via `SetNetworkConfigCb`
/// - Must be called exactly once per invocation
pub extern "C" fn set_network_config_done(context: *mut c_void, success: bool) {
    // SAFETY: context was created via Box::into_raw of a Box<oneshot::Sender<bool>>
    let sender = unsafe { Box::from_raw(context as *mut oneshot::Sender<bool>) };
    let _ = sender.send(success);
}

pub struct AppleOsImpl {
    receive_cb: extern "C" fn(FfiBytes),
    set_network_config_cb: SetNetworkConfigCb,
    network_interface: watch::Sender<Option<NetworkInterface>>,
}

impl AppleOsImpl {
    pub fn new(receive_cb: extern "C" fn(FfiBytes), set_network_config_cb: SetNetworkConfigCb) -> Self {
        Self { receive_cb, set_network_config_cb, network_interface: watch::channel(None).0 }
    }

    pub fn set_network_interface(&self, network_interface: Option<NetworkInterface>) {
        self.network_interface.send_replace(network_interface);
    }
}

impl Os for AppleOsImpl {
    fn network_interface(&self) -> watch::Receiver<Option<NetworkInterface>> {
        self.network_interface.subscribe()
    }

    async fn set_os_network_config(&self, network_config: OsNetworkConfig) -> Result<(), ()> {
        let json = serde_json::to_vec(&network_config).map_err(|error| {
            tracing::error!(message_id = "aP7xKm2q", ?error, "failed to serialize OsNetworkConfig");
        })?;

        let (tx, rx) = oneshot::channel();
        let context = Box::into_raw(Box::new(tx)) as *mut c_void;
        (self.set_network_config_cb)(json.ffi(), context, set_network_config_done);

        match rx.await {
            Ok(true) => Ok(()),
            Ok(false) => Err(()),
            Err(_) => {
                tracing::error!(
                    message_id = "bR3vNw8x",
                    "set_network_config done callback was dropped without being called"
                );
                Err(())
            }
        }
    }

    async fn unset_os_network_config(&self) -> Result<(), ()> {
        // Nothing to do. On Apple platform the OS manages this, not the PacketTunnelProvider implementation.
        Ok(())
    }

    fn packet_for_os(&self, packet: Bytes) {
        (self.receive_cb)(packet.as_ref().ffi());
    }
}
