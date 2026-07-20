use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio_util::task::AbortOnDropHandle;
use uuid::Uuid;
use zbus_systemd::zbus;

use super::ipc::{ClientError, run_command};
use super::status::{LinuxServiceDegradation, OsStatus, ServiceStatus};
use crate::manager::Status;
use crate::manager_cmd::ManagerCmd;

pub struct GuiStatusWatch {
    tx: watch::Sender<OsStatus>,
    _tasks: AbortOnDropHandle<()>,
}

impl GuiStatusWatch {
    pub async fn watch() -> Arc<Self> {
        let (tx, _) = watch::channel(OsStatus::default());
        let task = tokio::spawn(run_status_poller(tx.clone()));
        Arc::new(Self { tx, _tasks: AbortOnDropHandle::new(task) })
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
            Err(ClientError::NoService) => diagnose_no_service().await,
            Err(ClientError::InsufficientPermissions) => LinuxServiceDegradation::NoAccess,
            Ok(Err(error)) => {
                tracing::error!(message_id = "Jc2vZq8k", ?error, "service failed to get status");
                LinuxServiceDegradation::Other
            }
            Err(error) => {
                tracing::error!(message_id = "Xw5nRt3p", %error, "cannot reach service to get status");
                LinuxServiceDegradation::Other
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

async fn diagnose_no_service() -> LinuxServiceDegradation {
    let Ok(conn) = zbus::Connection::system().await else {
        return LinuxServiceDegradation::Other;
    };
    let Ok(systemd) = zbus_systemd::systemd1::ManagerProxy::new(&conn).await else {
        return LinuxServiceDegradation::Other;
    };
    match systemd.get_unit_file_state("obscura.service".to_owned()).await {
        Err(zbus::Error::MethodError(ref name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.FileNotFound" => {
            LinuxServiceDegradation::NotInstalled
        }
        Err(error) => {
            tracing::debug!(message_id = "Tk6wPd2j", %error, "could not classify service degradation");
            LinuxServiceDegradation::Other
        }
        Ok(state) if state == "disabled" => LinuxServiceDegradation::Disabled,
        Ok(_) => {
            if let Ok(path) = systemd.get_unit("obscura.service".to_owned()).await
                && let Ok(unit) = zbus_systemd::systemd1::UnitProxy::new(&conn, path).await
                && matches!(unit.active_state().await.as_deref(), Ok("failed"))
            {
                return LinuxServiceDegradation::Failed;
            }
            LinuxServiceDegradation::Stopped
        }
    }
}
