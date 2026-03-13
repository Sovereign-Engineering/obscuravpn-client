pub mod os;

use crate::ServiceArgs;
use std::convert::Infallible;

use anyhow::Context;
use obscuravpn_client::manager::Manager;
use obscuravpn_client::os::packet_buffer::PacketBuffer;
use std::error::Error;
use std::sync::Arc;

pub async fn run(args: ServiceArgs) -> Result<Infallible, Box<dyn Error>> {
    tracing::info!(message_id = "MNqPkSTH", "starting service");

    #[cfg(target_os = "linux")]
    let os_impl = os::linux::LinuxOsImpl::new(args.dns).await?;
    #[cfg(target_os = "windows")]
    let os_impl = os::windows::WindowsOsImpl::new().await?;

    let os_impl = Arc::new(os_impl);

    let manager = Manager::new(
        args.config_dir.into(),
        None,
        format!("obscura.net/{}/v0.0-alpha", std::env::consts::OS),
        tokio::runtime::Handle::current(),
        os_impl.clone(),
        None,
        None,
        false,
    )
    .context("failed to create manager")?;

    {
        let os_impl = os_impl.clone();
        let manager = manager.clone();
        tokio::spawn(async move {
            let mut packet_buffer = PacketBuffer::default();
            loop {
                os_impl.packets_for_relay(&mut packet_buffer).await;
                manager.packets_for_relay(packet_buffer.take_iter());
            }
        });
    }

    loop {
        let (cmd, response_fn) = os_impl.next_manager_command().await;
        let manager = manager.clone();
        tokio::spawn(async move { response_fn(cmd.run(&manager).await) });
    }
}
