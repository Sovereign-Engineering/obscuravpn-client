pub mod os;

use crate::ServiceArgs;

use anyhow::Context;
use obscuravpn_client::os::os_trait::{Os, RevocableOs};
use obscuravpn_client::version::release_version;
use obscuravpn_client::wg_key_store::WgKeyStore;
use obscuravpn_client::{logging::LogPersistence, manager::Manager};
use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// Runs the service with support to shut down from an external source
pub async fn run(
    args: ServiceArgs,
    log_persistence: Option<LogPersistence>,
    shutdown: Option<watch::Receiver<bool>>,
    scm_start_reason: Option<ScmStartReason>,
) -> Result<(), Box<dyn Error>> {
    tracing::info!(message_id = "MNqPkSTH", "starting service");

    let is_restart = detect_restart(args.runtime_dir.as_deref(), scm_start_reason);

    #[cfg(target_os = "linux")]
    let os_impl = os::linux::LinuxOsImpl::new(args.dns).await?;
    #[cfg(target_os = "windows")]
    let os_impl = os::windows::WindowsOsImpl::new().await?;

    let os_impl = Arc::new(os_impl);
    let manager_os_impl = Arc::new(RevocableOs::new(os_impl.clone()));

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
    let src_version = release_version().to_owned();
    let manager = Manager::new(
        args.config_dir.into(),
        wg_key_store,
        format!("obscura.net/{}/{src_version}", std::env::consts::OS),
        manager_os_impl.clone(),
        os_impl.network_interface(),
        log_persistence,
        !is_restart,
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
        "service shutdown requested; revoking OS network integration and reverting OS network configuration"
    );

    if tokio::time::timeout(Duration::from_secs(20), manager_os_impl.revoke()).await.is_err() {
        tracing::warn!(message_id = "cJ4tPz9V", "timed out revoking manager access to OS network integration");
    }
    if let Err(error) = os_impl.unset_os_network_config().await {
        tracing::warn!(message_id = "kN5bX1wz", ?error, "failed to revert OS network configuration on shutdown");
    }

    #[cfg(target_os = "windows")]
    let _ = os_impl.shutdown_wintun_session();

    Ok(())
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Clone, Copy, Debug)]
pub enum ScmStartReason {
    Auto,
    DelayedAuto,
    Demand,
    FailureRecovery,
    Trigger,
    Unknown,
}

fn detect_restart(runtime_dir: Option<&str>, scm_start_reason: Option<ScmStartReason>) -> bool {
    match scm_start_reason {
        Some(ScmStartReason::Demand | ScmStartReason::FailureRecovery) => {
            // Demand is used for explicit starts and restarts, including an installer triggered restart during update and FailureRecovery a restart after crash.
            tracing::info!(message_id = "wN4gXt7B", "potential windows service restart due to update or crash");
            return true;
        }
        Some(ScmStartReason::Auto | ScmStartReason::DelayedAuto | ScmStartReason::Trigger | ScmStartReason::Unknown) | None => {}
    }
    let Some(runtime_dir) = runtime_dir else {
        tracing::info!(message_id = "gV2sYh6M", "no runtime directory, assuming fresh service start");
        return false;
    };
    let marker = Path::new(runtime_dir).join("restart-marker");
    let is_restart = marker.exists();
    tracing::info!(message_id = "jW3qVb8N", is_restart, "checked service restart marker");
    if let Err(error) = File::create(&marker) {
        tracing::error!(message_id = "tK6dRp4X", ?error, "failed to create service restart marker");
    }
    is_restart
}
