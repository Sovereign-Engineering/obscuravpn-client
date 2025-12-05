// TODO: move this module to the library and make `Os` an argument to `Manager::new`
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod linux;
pub mod packet_buffer;

use crate::service::os::packet_buffer::PacketBuffer;
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode, ManagerCmdOk};
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::network_config::TunnelNetworkConfig;

pub trait Os {
    type PutIncomingPacketFn: PutIncomingPacketFn;

    /// Watcher for the network interface API requests and tunnels should use.
    fn network_interface(&self) -> tokio::sync::watch::Receiver<Option<NetworkInterface>>;

    /// Set the network state. Returning `Ok()` implies that the OS will route traffic to the tunnel. May be called repeatedly before the tunnel is functional or after the tunnel started relaying traffic to reflect changing IP Address or DNS configuration. Regardless of errors that may occur, the implementation should set up as much routing/filtering as possible to avoid leaking traffic.
    async fn set_tunnel_network_config(&mut self, network_config: TunnelNetworkConfig) -> Result<(), ()>;

    /// Reset the network state. Returning `Ok()` implies that the OS will stop routing traffic to the tunnel soon.
    // TODO: Merge with `set_tunnel_network_config` if allowing this to block turns out to be sensible (especially on Apple platforms) or delete this comment.
    async fn unset_tunnel_network_config(&mut self) -> Result<(), ()>;

    /// Use returned function to pass incoming IP packets, which should be emitted by the tunnel device.
    fn put_incoming_packet_fn(&self) -> Self::PutIncomingPacketFn;

    /// Get outgoing IP packets, which should be sent through the tunnel. May block until traffic is available.
    async fn get_outgoing_packets(&self, packet_buffer: &mut PacketBuffer);

    /// Get next manager command. Blocks until a command is available. The response function is guaranteed to be called when command processing completes.
    async fn get_manager_command(&self) -> (ManagerCmd, Box<dyn FnOnce(Result<ManagerCmdOk, ManagerCmdErrorCode>) + Send>);
}

// TODO: Remove in favor of [std::ops::Fn](https://doc.rust-lang.org/std/ops/trait.Fn.html) once implementing it is possible in stable.
pub trait PutIncomingPacketFn {
    fn call(&mut self, packet: &[u8]);
}
