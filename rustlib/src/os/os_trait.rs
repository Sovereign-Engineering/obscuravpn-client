use crate::net::NetworkInterface;
use crate::network_config::OsNetworkConfig;
use bytes::Bytes;

pub trait Os: Sync + Send + 'static {
    /// Watcher for the network interface API requests and tunnels should use. This method may be called multiple times. All returned watchers must receive updates until dropped.
    fn network_interface(&self) -> tokio::sync::watch::Receiver<Option<NetworkInterface>>;

    /// Set the network state. Returning `Ok()` implies that the OS will route traffic to the tunnel. May be called repeatedly before the tunnel is functional or after the tunnel started relaying traffic to reflect changing IP Address or DNS configuration. Regardless of errors that may occur, the implementation should set up as much routing/filtering as possible to avoid leaking traffic.
    /// Will not be called concurrently with itself or `unset_os_network_config`.
    // TODO: Consider moving this to its own trait with `&mut` receiver and remove sentence above.
    fn set_os_network_config(&self, network_config: OsNetworkConfig) -> impl Future<Output = Result<(), ()>> + Send;

    /// Reset the network state. Returning `Ok()` implies that the OS will stop routing traffic to the tunnel soon.
    fn unset_os_network_config(&self) -> impl Future<Output = Result<(), ()>> + Send;

    /// Will be called when a packet from the relay is received on the tunnel, which should be emitted on the tunnel device.
    fn packet_for_os(&self, packet: Bytes);
}
