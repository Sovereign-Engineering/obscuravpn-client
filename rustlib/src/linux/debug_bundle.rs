use super::status::DebugBundleStatus;
use crate::debug_bundle::client::{populate_client_debug_bundle, zip_and_remove_dir};
use crate::debug_bundle::daemon::DaemonDebugBundleHandle;
use crate::debug_bundle::{DIR_PREFIX, make_private_temp_dir, try_copy_dir_contents_recursive};
use crate::linux::ipc::run_command;
use crate::manager_cmd::ManagerCmd;
use camino::{Utf8Path, Utf8PathBuf};
use chrono::{SecondsFormat, Utc};
use tokio::sync::watch;

pub struct GuiDebugBundler {
    client_log_dir: Option<Utf8PathBuf>,
    status: watch::Sender<DebugBundleStatus>,
}

impl GuiDebugBundler {
    pub fn new(client_log_dir: Option<Utf8PathBuf>) -> Self {
        Self { client_log_dir, status: watch::channel(DebugBundleStatus::default()).0 }
    }

    pub fn subscribe(&self) -> watch::Receiver<DebugBundleStatus> {
        self.status.subscribe()
    }

    pub async fn create(&self, user_feedback: String) -> Result<Utf8PathBuf, ()> {
        let claimed = self.status.send_if_modified(|status| {
            if status.in_progress {
                return false;
            }
            *status = DebugBundleStatus { in_progress: true, latest_path: None, in_progress_counter: 1 };
            true
        });
        if !claimed {
            tracing::info!(message_id = "wQ7bJs4H", "debug bundle already in progress, rejecting request");
            return Err(());
        }

        let result = create_combined_debug_bundle(user_feedback, self.client_log_dir.as_deref()).await;

        self.status.send_modify(|status| {
            *status = DebugBundleStatus {
                in_progress: false,
                latest_path: result.as_ref().ok().map(ToString::to_string),
                in_progress_counter: 0,
            };
        });
        result
    }
}

async fn create_combined_debug_bundle(user_feedback: String, client_log_dir: Option<&Utf8Path>) -> Result<Utf8PathBuf, ()> {
    let work_dir = make_private_temp_dir().await?;
    let staging = work_dir.join("staging");
    tokio::fs::create_dir(&staging)
        .await
        .map_err(|error| tracing::error!(message_id = "sK8dQv3B", ?error, %staging, "failed to create debug bundle staging dir"))?;

    match run_command::<DaemonDebugBundleHandle>(ManagerCmd::CreateDaemonDebugBundle {}).await {
        Ok(Ok(DaemonDebugBundleHandle { path, token })) => {
            try_copy_dir_contents_recursive(&path, &staging).await;
            tracing::info!(message_id = "tS9bWk5H", %path, "copied daemon debug bundle into staging");
            match run_command::<()>(ManagerCmd::DeleteDaemonDebugBundle { token }).await {
                Ok(Ok(())) => tracing::info!(message_id = "gN4xQd8V", "daemon deleted its debug bundle dir"),
                Ok(Err(error)) => tracing::error!(message_id = "uD9gYm4R", ?error, "daemon failed to delete daemon debug bundle"),
                Err(error) => tracing::error!(message_id = "aK6pWv3T", ?error, "failed to send delete daemon debug bundle command"),
            }
        }
        Ok(Err(error)) => tracing::error!(message_id = "eW5jTq8B", ?error, "daemon failed to create daemon debug bundle"),
        Err(error) => tracing::error!(message_id = "hN2sXf7L", ?error, "failed to send create daemon debug bundle command"),
    }

    let user_feedback = (!user_feedback.is_empty()).then_some(user_feedback.as_str());
    populate_client_debug_bundle(&staging, user_feedback, client_log_dir).await;

    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true).replace(':', "-");
    zip_and_remove_dir(&staging, &work_dir, format!("{DIR_PREFIX}{timestamp}")).await
}
