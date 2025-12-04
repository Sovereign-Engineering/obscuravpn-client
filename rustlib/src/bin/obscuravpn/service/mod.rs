mod os;

use crate::ServiceArgs;
use std::convert::Infallible;

use crate::service::os::linux::LinuxOsImpl;
use crate::service::os::packet_buffer::PacketBuffer;
use crate::service::os::{Os, PutIncomingPacketFn};
use obscuravpn_api::types::AccountId;
use obscuravpn_client::exit_selection::ExitSelector;
use obscuravpn_client::ffi_helpers::FfiBytes;
use obscuravpn_client::manager::{Manager, Status, TunnelArgs, VpnStatus};
use obscuravpn_client::manager_cmd::ManagerCmd;
use obscuravpn_client::network_config::TunnelNetworkConfig;
use os::linux::tun::TunWriter;
use std::default::Default;
use std::sync::Mutex;
use tokio::select;

static TUN_WRITER: Mutex<TunWriter> = Mutex::new(TunWriter::invalid());

extern "C" fn receive_cb(packet: FfiBytes) {
    TUN_WRITER.lock().unwrap().call(packet.as_slice())
}

pub fn run(args: ServiceArgs) -> anyhow::Result<Infallible> {
    let runtime = tokio::runtime::Runtime::new()?;

    let mut init_commands = Vec::new();
    if let Some(account) = args.account {
        init_commands.push(ManagerCmd::Login { account_id: AccountId::from_string_unchecked(account), validate: true });
    }
    if args.connect {
        init_commands.push(ManagerCmd::SetTunnelArgs { args: Some(TunnelArgs { exit: ExitSelector::Any {} }), allow_activation: true });
    }

    let mut os_impl = LinuxOsImpl::new(runtime.handle(), init_commands)?;
    *TUN_WRITER.lock().expect("poisoned") = os_impl.put_incoming_packet_fn();

    let manager = Manager::new(
        args.config_dir.into(),
        None,
        "obscura.net/linux/v0.0-alpha".to_string(),
        &runtime,
        receive_cb,
        None,
        None,
    )?;

    // TODO: move into `Manager`
    runtime.block_on(async move {
        let mut status = manager.subscribe();
        status.mark_changed();

        let mut network_interface = os_impl.network_interface();

        let mut packet_buffer = PacketBuffer::default();

        loop {
            select! {
                biased;

                _ = network_interface.changed() => manager.set_network_interface(network_interface.borrow().clone()),

                (cmd, res_fn) = os_impl.get_manager_command() => {
                    let manager = manager.clone();
                    tokio::spawn(async move {
                        res_fn(cmd.run(&manager).await);
                    });
                }

                () = os_impl.get_outgoing_packets(&mut packet_buffer) => {
                    for packet in packet_buffer.take_iter() {
                        manager.send_packet(packet)
                    }
                }

                // TODO: remove in favor of calling appropriate `os_impl` methods in the right places
                _ = status.changed() => {
                    _ = dbg!(status.has_changed());
                    process_status_update(&status.borrow(), &mut os_impl).await;
                }
            }
        }
    })
}

async fn process_status_update(status: &Status, os_impl: &mut impl Os) {
    let network_config = match &status.vpn_status {
        VpnStatus::Connecting { .. } => Some(TunnelNetworkConfig::dummy()),
        VpnStatus::Connected { network_config, .. } => Some(network_config.clone()),
        VpnStatus::Disconnected { .. } => None,
    };
    match network_config {
        Some(network_config) => {
            if let Err(error) = os_impl.set_tunnel_network_config(network_config).await {
                // TODO: should become tunnel connect error published in status
                tracing::error!(message_id = "SisqqS5i", ?error, "set_tunnel_network_config failed");
            }
        }
        None => {
            if let Err(error) = os_impl.unset_tunnel_network_config().await {
                tracing::error!(message_id = "NYCr11HH", ?error, "unset_tunnel_network_config failed");
            }
        }
    }
}
