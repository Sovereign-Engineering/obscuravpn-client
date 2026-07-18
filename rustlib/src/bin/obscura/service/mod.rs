pub mod os;

use crate::ServiceArgs;
use std::convert::Infallible;

use anyhow::Context;
use obscuravpn_client::wg_key_store::WgKeyStore;
use obscuravpn_client::{logging::LogPersistence, manager::Manager};
use std::error::Error;
use std::sync::Arc;

pub async fn run(args: ServiceArgs, log_persistence: Option<LogPersistence>) -> Result<Infallible, Box<dyn Error>> {
    tracing::info!(message_id = "MNqPkSTH", "starting service");

    #[cfg(target_os = "linux")]
    let os_impl = os::linux::LinuxOsImpl::new(args.dns).await?;
    #[cfg(target_os = "windows")]
    let os_impl = os::windows::WindowsOsImpl::new().await?;

    let os_impl = Arc::new(os_impl);

    let wg_key_store = match WgKeyStore::sealed().await {
        Ok(wg_key_store) => wg_key_store,
        #[cfg(target_os = "linux")]
        Err(()) => {
            tracing::warn!(message_id = "Vt8mJc5R", "TPM sealing unavailable, storing the wireguard key in plaintext");
            WgKeyStore::Plaintext
        }
        #[cfg(target_os = "windows")]
        Err(()) => {
            tracing::warn!(message_id = "Bq4xNw7L", "TPM sealing unavailable, keeping the wireguard key in memory");
            WgKeyStore::None
        }
    };

    let manager = Manager::new(
        args.config_dir.into(),
        wg_key_store,
        format!("obscura.net/{}/v0.0-alpha", std::env::consts::OS),
        os_impl.clone(),
        log_persistence,
        false,
    )
    .context("failed to create manager")?;

    loop {
        let (cmd, response_fn) = os_impl.next_manager_command().await;
        let manager = manager.clone();
        tokio::spawn(async move { response_fn(cmd.run(&manager).await) });
    }
}
