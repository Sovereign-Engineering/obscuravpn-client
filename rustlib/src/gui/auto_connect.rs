use std::sync::Arc;

use obscuravpn_client::linux::ipc::run_command;
use obscuravpn_client::linux::status::ServiceStatus;
use obscuravpn_client::linux::status_watch::GuiStatusWatch;
use obscuravpn_client::manager::{TunnelArgs, VpnStatus};
use obscuravpn_client::manager_cmd::ManagerCmd;

pub async fn auto_connect_if_enabled(gui_status: Arc<GuiStatusWatch>) {
    let mut known_version = None;
    let status = loop {
        let os_status = gui_status.changed(known_version).await;
        known_version = Some(os_status.version);
        match os_status.service_status {
            ServiceStatus::Healthy(status) => break status,
            ServiceStatus::Initializing | ServiceStatus::Degraded { last_status: _, linux_degradation: _ } => {}
        }
    };
    if !status.auto_connect {
        tracing::info!(message_id = "Fq3wNd7K", "auto-connect disabled");
        return;
    }
    match status.vpn_status {
        VpnStatus::Disconnected {} => {}
        VpnStatus::Connecting { .. } | VpnStatus::Connected { .. } => {
            tracing::info!(message_id = "Hs6tRb2M", "tunnel already active, skipping auto-connect");
            return;
        }
    }
    let exit = status.last_exit;
    tracing::info!(message_id = "Jc8vLp4W", ?exit, "auto-connecting");
    let cmd = ManagerCmd::SetTunnelArgs { args: Some(TunnelArgs { exit }), active: Some(true) };
    match run_command::<serde_json::Value>(cmd).await {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => tracing::error!(message_id = "Nz5kTq9X", ?error, "auto-connect failed"),
        Err(error) => tracing::error!(message_id = "Rd2mYf7B", ?error, "auto-connect failed"),
    }
}
