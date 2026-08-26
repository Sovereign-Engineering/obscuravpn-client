use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::watch;
use tokio_util::task::AbortOnDropHandle;
use uuid::Uuid;

use super::ipc::{LinuxIpcError, run_command};
use super::status::{DebugBundleStatus, LinuxServiceDegradation, NavigationView, OsStatus, ServiceStatus};
use super::systemd::SystemdUnitStatus;
use super::{argv0, autostart, current_user_name};
use crate::manager::Status;
use crate::manager_cmd::ManagerCmd;
use crate::version::release_version;

pub struct GuiStatusWatch {
    tx: watch::Sender<OsStatus>,
    _tasks: [AbortOnDropHandle<()>; 2],
}

impl GuiStatusWatch {
    pub async fn watch(debug_bundle_status: watch::Receiver<DebugBundleStatus>) -> Arc<Self> {
        let (tx, _) = watch::channel(OsStatus::new(autostart::autostart_status().await));
        let poller = tokio::spawn(run_status_poller(tx.clone()));
        let forwarder = tokio::spawn(forward_debug_bundle_status(tx.clone(), debug_bundle_status));
        Arc::new(Self { tx, _tasks: [AbortOnDropHandle::new(poller), AbortOnDropHandle::new(forwarder)] })
    }

    pub fn current(&self) -> OsStatus {
        self.tx.borrow().clone()
    }

    pub fn set_navigation_view(&self, view: NavigationView) {
        self.tx.send_if_modified(|os_status| {
            let version = os_status.version;
            os_status.set_navigation_view(view);
            os_status.version != version
        });
    }

    pub async fn refresh_login_item_status(&self) {
        let status = autostart::autostart_status().await;
        self.tx.send_if_modified(|os_status| {
            let version = os_status.version;
            os_status.set_login_item_status(status);
            os_status.version != version
        });
    }

    pub async fn changed(&self, known_version: Option<Uuid>) -> OsStatus {
        self.tx
            .subscribe()
            .wait_for(|os_status| Some(os_status.version) != known_version)
            .await
            .expect("sender held by self")
            .clone()
    }
}

async fn forward_debug_bundle_status(tx: watch::Sender<OsStatus>, mut rx: watch::Receiver<DebugBundleStatus>) {
    loop {
        let status = rx.borrow_and_update().clone();
        tx.send_if_modified(|os_status| {
            let version = os_status.version;
            os_status.set_debug_bundle_status(status);
            os_status.version != version
        });
        if rx.changed().await.is_err() {
            tracing::info!(message_id = "bT5wNc8K", "debug bundle status channel closed");
            return;
        }
    }
}

async fn run_status_poller(tx: watch::Sender<OsStatus>) {
    let mut known_version: Option<Uuid> = None;
    loop {
        let degradation = match run_command::<Status>(ManagerCmd::GetStatus { known_version }).await {
            Ok(Ok(status)) => {
                known_version = Some(status.version);
                tx.send_if_modified(|os_status| {
                    let version = os_status.version;
                    os_status.set_service_status(ServiceStatus::Healthy(status));
                    os_status.version != version
                });
                continue;
            }
            Err(LinuxIpcError::NoListener) => classify_unreachable(ConnectFailure::NoListener).await,
            Err(LinuxIpcError::InsufficientPermissions) => classify_unreachable(ConnectFailure::InsufficientPermissions).await,
            Err(LinuxIpcError::VersionMismatch { service_version, app_version }) => {
                let installed_app_version_differs = installed_app_version_differs().await.ok();
                LinuxServiceDegradation::VersionMismatch { service_version, app_version, installed_app_version_differs }
            }
            Ok(Err(error)) => {
                tracing::error!(message_id = "Jc2vZq8k", ?error, "service failed to get status");
                LinuxServiceDegradation::Unknown
            }
            Err(LinuxIpcError::Other) => {
                tracing::error!(message_id = "Xw5nRt3p", "cannot reach service to get status");
                LinuxServiceDegradation::Unknown
            }
        };
        known_version = None;
        tx.send_if_modified(|os_status| {
            let version = os_status.version;
            let last_status = match os_status.service_status.clone() {
                ServiceStatus::Initializing => None,
                ServiceStatus::Healthy(status) => Some(status),
                ServiceStatus::Degraded { last_status, linux_degradation: _ } => last_status,
            };
            os_status.set_service_status(ServiceStatus::Degraded { last_status, linux_degradation: degradation });
            os_status.version != version
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn installed_app_version_differs() -> Result<bool, ()> {
    let Some(invocation_path) = argv0() else {
        tracing::error!(message_id = "wN3kFb7T", "cannot probe installed app version without argv[0]");
        return Err(());
    };
    let output = tokio::time::timeout(Duration::from_secs(2), Command::new(invocation_path).arg("version").output())
        .await
        .map_err(|error| tracing::error!(message_id = "cQ8mZj2R", %error, "installed app version probe timed out"))?
        .map_err(|error| tracing::error!(message_id = "hL5xVd9G", %error, "failed to run installed app version probe"))?;
    if !output.status.success() {
        let status = output.status;
        tracing::error!(message_id = "sB4tKp6W", %status, "installed app version probe failed");
        return Err(());
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| tracing::error!(message_id = "vT6qLm3D", %error, "installed app version probe printed invalid utf-8"))?;
    let installed_version = stdout.trim();
    let running_version = release_version();
    tracing::info!(
        message_id = "dF7wRb4N",
        installed_version,
        running_version,
        "probed installed app version"
    );
    Ok(installed_version != running_version)
}

enum ConnectFailure {
    NoListener,
    InsufficientPermissions,
}

async fn classify_unreachable(failure: ConnectFailure) -> LinuxServiceDegradation {
    match SystemdUnitStatus::get().await {
        SystemdUnitStatus::NotInstalled => LinuxServiceDegradation::UnitNotInstalled,
        SystemdUnitStatus::Unknown | SystemdUnitStatus::Active => match failure {
            ConnectFailure::InsufficientPermissions => LinuxServiceDegradation::SocketPermissionDenied { user: current_user_name().await },
            ConnectFailure::NoListener => LinuxServiceDegradation::Unknown,
        },
        SystemdUnitStatus::Activating | SystemdUnitStatus::Reloading | SystemdUnitStatus::Refreshing => LinuxServiceDegradation::UnitActivating,
        SystemdUnitStatus::Inactive | SystemdUnitStatus::Failed | SystemdUnitStatus::Deactivating => LinuxServiceDegradation::UnitInactive,
    }
}
