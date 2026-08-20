use std::time::Duration;

use obscuravpn_client::linux::current_user_name;
use obscuravpn_client::linux::systemd::{SystemdUnitStatus, UNIT_NAME};
use tokio::process::Command;
use tokio::time::Instant;

use crate::error::LinuxFixErrorCode;

pub async fn add_operator() -> Result<(), LinuxFixErrorCode> {
    let Some(user) = current_user_name().await else {
        return Err(LinuxFixErrorCode::UsernameUnknown);
    };
    run_pkexec(&["obscura", "add-operator", &user], LinuxFixErrorCode::AddOperatorFailed).await
}

pub async fn restart_service(enable: bool) -> Result<(), LinuxFixErrorCode> {
    if enable {
        run_pkexec(
            &[
                "sh",
                "-c",
                r#"systemctl enable --force "$1" && systemctl restart "$1""#,
                "obscura-enable-restart",
                UNIT_NAME,
            ],
            LinuxFixErrorCode::ServiceEnableAndRestartFailed,
        )
        .await?;
    } else {
        run_pkexec(&["systemctl", "restart", UNIT_NAME], LinuxFixErrorCode::ServiceRestartFailed).await?;
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut seen_activating = false;
    loop {
        let state = SystemdUnitStatus::get().await;
        match state {
            SystemdUnitStatus::Active => return Ok(()),
            SystemdUnitStatus::Failed if seen_activating => {
                tracing::error!(message_id = "yG3mQw7B", "service entered failed state after restart");
                return Err(LinuxFixErrorCode::ServiceStartFailed);
            }
            SystemdUnitStatus::Activating => seen_activating = true,
            SystemdUnitStatus::NotInstalled
            | SystemdUnitStatus::Unknown
            | SystemdUnitStatus::Reloading
            | SystemdUnitStatus::Refreshing
            | SystemdUnitStatus::Inactive
            | SystemdUnitStatus::Failed
            | SystemdUnitStatus::Deactivating => {}
        }
        if Instant::now() >= deadline {
            tracing::error!(message_id = "dK8sLr5N", %state, "service did not become active in time");
            return Err(LinuxFixErrorCode::ServiceStartTimeout);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn run_pkexec(args: &[&str], failure_code: LinuxFixErrorCode) -> Result<(), LinuxFixErrorCode> {
    let output = Command::new("pkexec").args(args).output().await.map_err(|error| {
        tracing::error!(message_id = "qF6vZj3W", %error, ?args, "failed to run pkexec");
        LinuxFixErrorCode::PkexecUnavailable
    })?;
    if output.status.success() {
        return Ok(());
    }
    let status = output.status;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    Err(match status.code() {
        Some(126) => {
            tracing::info!(message_id = "wR2kDn8Y", ?args, "authorization dialog was dismissed");
            LinuxFixErrorCode::AuthorizationDismissed
        }
        Some(127) => {
            tracing::error!(message_id = "jP5cVs4L", stderr, ?args, "pkexec authorization failed");
            LinuxFixErrorCode::AuthorizationDenied
        }
        _ => {
            tracing::error!(message_id = "nT9bXf2H", %status, stderr, ?args, "pkexec command failed");
            failure_code
        }
    })
}
