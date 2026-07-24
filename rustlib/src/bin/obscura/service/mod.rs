pub mod os;

use crate::ServiceArgs;

use anyhow::Context;
use obscuravpn_client::manager::VpnStatus;
use obscuravpn_client::os::os_trait::Os;
use obscuravpn_client::wg_key_store::WgKeyStore;
use obscuravpn_client::{logging::LogPersistence, manager::Manager};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// Runs the service with support to shut down from an external source
pub async fn run(args: ServiceArgs, log_persistence: Option<LogPersistence>, shutdown: Option<watch::Receiver<bool>>) -> Result<(), Box<dyn Error>> {
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
    let src_version = option_env!("OBSCURA_VERSION").unwrap_or("v0.0.0-dev").to_owned();
    let manager = Manager::new(
        args.config_dir.into(),
        wg_key_store,
        format!("obscura.net/{}/{src_version}", std::env::consts::OS),
        os_impl.clone(),
        log_persistence,
        false,
    )
    .context("failed to create manager")?;

    let mut shutdown = std::pin::pin!(async {
        match shutdown {
            Some(mut rx) => {
                let _ = rx.wait_for(|&stop| stop).await;
            }
            None => std::future::pending::<()>().await,
        }
    });

    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => break,
            (cmd, response_fn) = os_impl.next_manager_command() => {
                let manager = manager.clone();
                tokio::spawn(async move { response_fn(cmd.run(&manager).await) });
            }
        }
    }

    tracing::info!(
        message_id = "rT8yQ2dC",
        "service shutdown requested; disconnecting and reverting OS network configuration"
    );

    _ = manager.run_on_client_state(|client_state| client_state.set_tunnel_target_state(None, Some(false)));
    let mut status = manager.subscribe();
    let wait_till_disconnected = async {
        while !matches!(status.borrow_and_update().vpn_status, VpnStatus::Disconnected {}) {
            if let Err(error) = status.changed().await {
                tracing::error!(
                    message_id = "jFaRdAvk",
                    ?error,
                    "error while waiting for disconnected status for shutdown"
                );
                break;
            }
        }
    };
    if let Err(error) = tokio::time::timeout(Duration::from_secs(20), wait_till_disconnected).await {
        tracing::warn!(message_id = "cJ4tPz9V", ?error, "timed out waiting for manager to disconnect on shutdown");
    }
    if let Err(error) = os_impl.unset_os_network_config().await {
        tracing::warn!(message_id = "kN5bX1wz", ?error, "failed to revert OS network configuration on shutdown");
    }

    Ok(())
}
