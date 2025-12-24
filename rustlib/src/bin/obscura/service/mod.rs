pub mod os;

use crate::ServiceArgs;
use std::convert::Infallible;

use crate::service::os::linux::LinuxOsImpl;
use crate::service::os::linux::start_error::ServiceStartError;
use crate::service::os::packet_buffer::PacketBuffer;
use crate::service::os::{Os, PutIncomingPacketFn};
use anyhow::Context;
use obscuravpn_client::ffi_helpers::FfiBytes;
use obscuravpn_client::manager::{Manager, Status, VpnStatus};
use obscuravpn_client::network_config::TunnelNetworkConfig;
use os::linux::tun::TunWriter;
use std::default::Default;
use std::sync::Mutex;
use tokio::select;

static TUN_WRITER: Mutex<TunWriter> = Mutex::new(TunWriter::invalid());

extern "C" fn receive_cb(packet: FfiBytes) {
    TUN_WRITER.lock().unwrap().call(packet.as_slice())
}

pub async fn run(args: ServiceArgs) -> Result<Infallible, ServiceStartError> {
    tracing::info!(message_id = "MNqPkSTH", "starting service");

    let mut os_impl = LinuxOsImpl::new(args.dns).await?;
    *TUN_WRITER.lock().expect("poisoned") = os_impl.put_incoming_packet_fn();

    let manager = Manager::new(
        args.config_dir.into(),
        None,
        "obscura.net/linux/v0.0-alpha".to_string(),
        tokio::runtime::Handle::current(),
        receive_cb,
        None,
        None,
    )
    .context("failed to create manager")?;

    // TODO: move into `Manager`
    let mut status = manager.subscribe();
    status.mark_changed();

    let mut network_interface = os_impl.network_interface();

    let mut packet_buffer = PacketBuffer::default();

    loop {
        select! {
            biased;

            _ = network_interface.changed() => manager.set_network_interface(network_interface.borrow().clone()),

            (cmd, response_fn) = os_impl.get_manager_command() => {
                let manager = manager.clone();
                tokio::spawn(async move {
                    response_fn(cmd.run(&manager).await)
                });
            }

            () = os_impl.get_outgoing_packets(&mut packet_buffer) => {
                for packet in packet_buffer.take_iter() {
                    manager.send_packet(packet)
                }
            }

            // TODO: remove in favor of calling appropriate `os_impl` methods in the right places
            _ = status.changed() => {
                process_status_update(&status.borrow(), &mut os_impl).await;
            }
        }
    }
}

async fn process_status_update(status: &Status, os_impl: &mut impl Os) {
    let network_config = match &status.vpn_status {
        VpnStatus::Connecting { .. } => Some(TunnelNetworkConfig::dummy()),
        VpnStatus::Connected { network_config, .. } => Some(network_config.clone()),
        VpnStatus::Disconnected { .. } => None,
    };
    match network_config {
        Some(network_config) => {
            if let Err(()) = os_impl.set_tunnel_network_config(network_config).await {
                // TODO: should become tunnel connect error published in status
                tracing::error!(message_id = "SisqqS5i", "set_tunnel_network_config failed");
            }
        }
        None => {
            if let Err(()) = os_impl.unset_tunnel_network_config().await {
                tracing::error!(message_id = "NYCr11HH", "unset_tunnel_network_config failed");
            }
        }
    }
}
